//! 时间领域的 CLI 适配：环境变量和持久配置只在确实需要默认时区时读取。

use super::{DateFormat, Report, Unit, Zone, parse_datetime, parse_timestamp, report};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Args)]
#[command(after_help = r#"使用案例:
  查看上海当前时间
    kito time now --timezone Asia/Shanghai
  毫秒时间戳转日期时间
    kito time from-timestamp 1704067200123 --unit ms --timezone Asia/Shanghai
  上海日期时间转微秒时间戳
    kito time to-timestamp "2024-01-01 08:00:00.123456" --timezone Asia/Shanghai --unit us --value-only
  将上海时间转换为纽约时间
    kito time convert "2024-01-01 08:00:00" --from-timezone Asia/Shanghai --to-timezone America/New_York
  按名称查询全球时区
    kito time zones Shanghai --at "2024-07-01T00:00:00Z"
  查看默认时区及其来源
    kito time timezone get --json"#)]
pub struct TimeArgs {
    #[command(subcommand)]
    command: TimeCommand,
}

#[derive(Subcommand)]
enum TimeCommand {
    #[command(
        about = "查看当前时间；--unit 指定时间戳结果的单位",
        after_help = r#"使用案例:
  查看指定时区的当前日期时间
    kito time now --timezone Asia/Shanghai
  只输出当前毫秒时间戳
    kito time now --timezone UTC --unit ms --value-only
  按模板展示日期，同时输出完整 JSON 报告
    kito time now --timezone Asia/Shanghai --format "%Y-%m-%d %H:%M:%S%.6f%:z" --json"#
    )]
    Now {
        #[arg(
            long,
            value_enum,
            help = "时间戳结果单位：s 秒、ms 毫秒、us 微秒、ns 纳秒；省略时主结果为日期时间，降低精度时向负无穷取整"
        )]
        unit: Option<Unit>,
        #[command(flatten)]
        zone: ZoneArgs,
        #[command(flatten)]
        output: DateOutputArgs,
    },
    #[command(
        about = "Unix 时间戳转日期时间，支持 s/ms/us/ns",
        after_help = r#"使用案例:
  将毫秒时间戳转换为上海日期时间
    kito time from-timestamp 1704067200123 --unit ms --timezone Asia/Shanghai
  自动识别微秒时间戳，并按模板只输出日期时间
    kito time from-timestamp 1704067200123456 --timezone Asia/Shanghai --format "%Y-%m-%d %H:%M:%S%.6f%:z" --value-only
  明确指定纪元前小值的单位，输出完整 JSON 报告
    kito time from-timestamp -1 --unit ms --timezone UTC --json"#
    )]
    FromTimestamp {
        #[arg(
            allow_hyphen_values = true,
            value_name = "VALUE",
            help = "Unix 时间戳整数，可带正负号；实际输入单位由 --unit 指定或按位数识别"
        )]
        value: String,
        #[arg(long, default_value = "auto", value_parser = input_unit, help = "输入单位：s 秒、ms 毫秒、us 微秒、ns 纳秒；auto 忽略符号和前导零，按 1–10/13/16/19 位识别；历史值建议显式指定")]
        unit: String,
        #[command(flatten)]
        zone: ZoneArgs,
        #[command(flatten)]
        output: DateOutputArgs,
    },
    #[command(
        about = "日期时间转 Unix 时间戳；降低精度向负无穷取整",
        after_help = r#"使用案例:
  将上海本地日期时间转换为微秒时间戳
    kito time to-timestamp "2024-01-01 08:00:00.123456" --timezone Asia/Shanghai --unit us --value-only
  使用输入自带的 offset，输出毫秒时间戳的完整报告
    kito time to-timestamp "2024-01-01T08:00:00.123456+08:00" --unit ms --json
  输出秒时间戳，并自定义报告中的日期展示
    kito time to-timestamp "2024-01-01T00:00:00Z" --unit s --format "%Y/%m/%d %H:%M:%S%:z""#
    )]
    ToTimestamp {
        #[arg(
            value_name = "DATETIME",
            help = "日期时间：RFC 3339 或 YYYY-MM-DD HH:MM:SS[.小数秒]（最多 9 位）；含空格时加引号，自带 offset 时优先使用它"
        )]
        datetime: String,
        #[arg(
            long,
            value_enum,
            default_value = "s",
            help = "时间戳结果单位：s 秒、ms 毫秒、us 微秒、ns 纳秒；降低精度时向负无穷取整，并在完整报告中标记 precision_lost"
        )]
        unit: Unit,
        #[command(flatten)]
        zone: ZoneArgs,
        #[command(flatten)]
        output: DateOutputArgs,
    },
    #[command(
        about = "将日期时间从源时区转换到目标时区",
        after_help = r#"使用案例:
  将上海本地时间转换为纽约时间
    kito time convert "2024-01-01 08:00:00" --from-timezone Asia/Shanghai --to-timezone America/New_York --value-only
  将带 offset 的日期时间转换为固定偏移，输出 JSON
    kito time convert "2024-01-01T00:00:00Z" --to-timezone +05:45 --json
  使用默认目标时区，并按模板只输出日期时间
    kito time convert "2024-01-01T00:00:00Z" --format "%Y-%m-%d %H:%M:%S%:z" --value-only"#
    )]
    Convert {
        #[arg(
            value_name = "DATETIME",
            help = "待转换的日期时间：RFC 3339 或 YYYY-MM-DD HH:MM:SS[.小数秒]（最多 9 位）；含空格时加引号，自带 offset 时优先用于解释源时间"
        )]
        datetime: String,
        #[arg(
            long,
            value_name = "ZONE",
            allow_hyphen_values = true,
            help = "源 IANA 时区或固定偏移；省略时先用输入 offset，再按 KITO_TIMEZONE、保存配置、UTC 选择；显式指定时须与输入 offset 一致"
        )]
        from_timezone: Option<Zone>,
        #[arg(
            long,
            value_name = "ZONE",
            allow_hyphen_values = true,
            help = "目标 IANA 时区或固定偏移；省略时按 KITO_TIMEZONE、保存配置、UTC 选择"
        )]
        to_timezone: Option<Zone>,
        #[command(flatten)]
        output: DateOutputArgs,
    },
    #[command(
        about = "列出全球 IANA 时区（含别名），可按名称、偏移或缩写筛选",
        after_help = r#"使用案例:
  按名称查询时区及当前偏移
    kito time zones Shanghai
  只输出匹配的 IANA 时区名，每行一个
    kito time zones America --value-only
  查询指定参考时刻的偏移和缩写
    kito time zones New_York --at "2024-07-01T00:00:00Z" --json
  将负偏移筛选值放在 -- 后面
    kito time zones --value-only -- -05:00"#
    )]
    Zones {
        #[arg(
            value_name = "FILTER",
            help = "按名称、偏移或缩写筛选，忽略大小写；例如 Shanghai、America、+05:45；负偏移用 -- -05:00"
        )]
        filter: Option<String>,
        #[arg(
            long,
            value_name = "DATETIME",
            help = "计算偏移的参考时刻，须包含 offset；默认现在，例 2024-07-01T00:00:00Z"
        )]
        at: Option<String>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(
        about = "保存、查看或清除默认时区，重开终端后仍生效",
        after_help = r#"使用案例:
  持久保存上海为默认时区
    kito time timezone set Asia/Shanghai
  查看当前生效的默认时区及其来源
    kito time timezone get --json
  清除保存的默认时区，回退到 KITO_TIMEZONE 或 UTC
    kito time timezone reset --json"#
    )]
    Timezone {
        #[command(subcommand)]
        command: TimezoneCommand,
    },
}

#[derive(Subcommand)]
enum TimezoneCommand {
    #[command(
        about = "持久保存默认时区；KITO_TIMEZONE 和显式参数仍可覆盖",
        after_help = r#"使用案例:
  将上海保存为后续命令的默认时区
    kito time timezone set Asia/Shanghai
  保存固定偏移，并输出 JSON 配置报告
    kito time timezone set +05:45 --json"#
    )]
    Set {
        #[arg(
            value_name = "ZONE",
            allow_hyphen_values = true,
            help = "要保存的 IANA 时区或固定偏移，例如 Asia/Shanghai、UTC、+08:00、-03:30"
        )]
        timezone: Zone,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(
        about = "查看当前生效的默认时区及来源",
        after_help = r#"使用案例:
  仅输出当前生效的默认时区名称
    kito time timezone get --value-only
  查看默认时区、来源与配置路径
    kito time timezone get --json"#
    )]
    Get {
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(
        about = "清除已保存的时区，回退到 KITO_TIMEZONE 或 UTC",
        after_help = r#"使用案例:
  清除保存的默认时区，并输出 JSON 操作报告
    kito time timezone reset --json
  清除保存项，成功后仅输出 reset
    kito time timezone reset --value-only"#
    )]
    Reset {
        #[command(flatten)]
        output: OutputArgs,
    },
}

#[derive(Args)]
struct ZoneArgs {
    #[arg(
        long,
        value_name = "ZONE",
        allow_hyphen_values = true,
        help = "IANA 时区或固定偏移；默认 KITO_TIMEZONE > 保存配置 > UTC。to-timestamp 中为源时区，输入自带 offset 时优先使用它，显式时区须与其一致；其他命令中为目标时区"
    )]
    timezone: Option<Zone>,
}

#[derive(Args)]
struct OutputArgs {
    #[arg(
        long,
        conflicts_with = "value_only",
        help = "输出完整 JSON 报告，与 --value-only 互斥；时间转换中的时间戳用字符串表示以保留全部精度"
    )]
    json: bool,
    #[arg(
        long,
        help = "仅输出主结果：时间转换为日期或时间戳，zones 为每行一个时区名，timezone set/get 为时区名、reset 为 reset；与 --json 互斥"
    )]
    value_only: bool,
}

#[derive(Args)]
struct DateOutputArgs {
    #[arg(
        short = 'f',
        long,
        value_name = "PATTERN",
        help = "仅改变日期展示的 strftime 模板，例如 \"%Y-%m-%d %H:%M:%S%.6f%:z\"；%.3f/%.6f/%.9f 控制小数位数，%:z 显示时区偏移；不改变时间戳主结果、JSON 完整 UTC/时间戳字段或 precision_lost"
    )]
    format: Option<DateFormat>,
    #[command(flatten)]
    mode: OutputArgs,
}

fn input_unit(value: &str) -> Result<String, String> {
    if value != "auto" {
        Unit::from_str(value, false)?;
    }
    Ok(value.to_owned())
}

fn configured_zone(explicit: Option<Zone>) -> Result<Zone> {
    if let Some(zone) = explicit {
        return Ok(zone);
    }
    Ok(super::config::effective()?.zone)
}

fn source_datetime(input: &str, explicit: Option<Zone>) -> Result<(chrono::DateTime<Utc>, Zone)> {
    let parsed = parse_datetime(input)?;
    // 显式源时区需要和输入 offset 校验；环境及保存配置只补足没有 offset 的输入。
    // 延迟读取默认值也让显式参数能覆盖错误的环境变量或损坏的配置。
    let source = match explicit.or_else(|| parsed.embedded_zone()) {
        Some(source) => source,
        None => configured_zone(None)?,
    };
    Ok((parsed.resolve(&source)?, source))
}

fn emit(report: &Report, output: &OutputArgs) -> Result<()> {
    if output.json {
        println!("{}", serde_json::to_string(report)?);
    } else if output.value_only {
        println!("{}", report.value);
    } else {
        println!("结果：{}", report.value);
        if report.value != report.datetime {
            println!("日期时间：{}", report.datetime);
        }
        println!(
            "时区：{}（{}，{}）",
            report.timezone, report.utc_offset, report.abbreviation
        );
        println!("UTC：{}", report.utc);
        println!("Unix 秒：{}", report.timestamps.s);
        println!("Unix 毫秒：{}", report.timestamps.ms);
        println!("Unix 微秒：{}", report.timestamps.us);
        println!("Unix 纳秒：{}", report.timestamps.ns);
        if let Some(unit) = report.unit {
            println!("时间戳单位：{}", unit.name());
        }
        if report.precision_lost {
            println!("精度说明：结果已按所选单位向负无穷取整，完整精度见 UTC 日期时间和纳秒字段。");
        }
    }
    Ok(())
}

fn timezone_command(command: TimezoneCommand) -> Result<bool> {
    let (report, output) = match command {
        TimezoneCommand::Set { timezone, output } => (super::config::set(&timezone)?, output),
        TimezoneCommand::Get { output } => (super::config::get()?, output),
        TimezoneCommand::Reset { output } => (super::config::reset()?, output),
    };
    if output.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if output.value_only {
        println!("{}", report.timezone.as_deref().unwrap_or("reset"));
    } else {
        match report.action {
            "set" => println!(
                "已保存默认时区：{}（单次参数和 KITO_TIMEZONE 可覆盖）",
                report.timezone.as_deref().unwrap_or("")
            ),
            "reset" => println!("已清除保存的默认时区；后续使用 KITO_TIMEZONE，未设置时使用 UTC。"),
            _ => {
                let source = match report.source {
                    Some("environment") => "KITO_TIMEZONE 环境变量",
                    Some("config") => "保存的配置",
                    _ => "内置 UTC 后备值",
                };
                println!(
                    "默认时区：{}（来源：{source}）",
                    report.timezone.as_deref().unwrap_or("")
                );
            }
        }
        if let Some(path) = report.config_path {
            println!("配置文件：{}", path.display());
        }
    }
    Ok(true)
}

pub fn run(args: TimeArgs) -> Result<bool> {
    let (report, output) = match args.command {
        TimeCommand::Now { unit, zone, output } => {
            let zone = configured_zone(zone.timezone)?;
            (
                report(
                    "now",
                    Utc::now(),
                    &zone,
                    unit,
                    unit.is_some(),
                    output.format.as_ref(),
                )?,
                output.mode,
            )
        }
        TimeCommand::FromTimestamp {
            value,
            unit,
            zone,
            output,
        } => {
            let unit = if unit == "auto" {
                None
            } else {
                Some(Unit::from_str(&unit, false).map_err(anyhow::Error::msg)?)
            };
            let (instant, unit) = parse_timestamp(&value, unit)?;
            let zone = configured_zone(zone.timezone)?;
            (
                report(
                    "from-timestamp",
                    instant,
                    &zone,
                    Some(unit),
                    false,
                    output.format.as_ref(),
                )?,
                output.mode,
            )
        }
        TimeCommand::ToTimestamp {
            datetime,
            unit,
            zone,
            output,
        } => {
            let (instant, source) = source_datetime(&datetime, zone.timezone)?;
            (
                report(
                    "to-timestamp",
                    instant,
                    &source,
                    Some(unit),
                    true,
                    output.format.as_ref(),
                )?,
                output.mode,
            )
        }
        TimeCommand::Convert {
            datetime,
            from_timezone,
            to_timezone,
            output,
        } => {
            let (instant, _) = source_datetime(&datetime, from_timezone)?;
            let to_timezone = configured_zone(to_timezone)?;
            (
                report(
                    "convert",
                    instant,
                    &to_timezone,
                    None,
                    false,
                    output.format.as_ref(),
                )?,
                output.mode,
            )
        }
        TimeCommand::Zones { filter, at, output } => {
            let list = super::zones::list(filter.as_deref(), at.as_deref())?;
            if output.json {
                println!("{}", serde_json::to_string(&list)?);
            } else if output.value_only {
                for zone in &list.zones {
                    println!("{}", zone.name);
                }
            } else {
                println!(
                    "全球时区：{} 项（含 IANA 别名；tzdb {}）",
                    list.zones.len(),
                    list.tzdb_version
                );
                println!(
                    "偏移参考时刻：{}；夏令时和历史规则可能使其他日期的偏移不同。",
                    list.reference_utc
                );
                println!("{:<32} {:<12} 缩写", "IANA name", "UTC offset");
                for zone in &list.zones {
                    println!(
                        "{:<32} {:<12} {}",
                        zone.name, zone.utc_offset, zone.abbreviation
                    );
                }
                if list.zones.is_empty() {
                    println!("没有匹配的时区；可运行 kito time zones 查看完整列表。");
                }
            }
            return Ok(true);
        }
        TimeCommand::Timezone { command } => return timezone_command(command),
    };
    emit(&report, &output)?;
    Ok(true)
}
