//! Windows 私钥目录使用当前用户 SID 的受保护 DACL；不依赖 shell 或 readonly。
use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetNamedSecurityInfoW, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION, GetAce, GetSecurityDescriptorControl,
    GetTokenInformation, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn check(value: i32) -> io::Result<()> {
    if value == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
unsafe fn sid_string(sid: *mut c_void) -> io::Result<String> {
    let mut pointer = null_mut();
    unsafe {
        check(ConvertSidToStringSidW(sid, &mut pointer))?;
    }
    let _allocated = Local(pointer.cast());
    let mut count = 0;
    unsafe {
        while *pointer.add(count) != 0 {
            count += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(pointer, count)).map_err(io::Error::other)
    }
}
fn current_sid() -> io::Result<String> {
    unsafe {
        let mut token = null_mut();
        check(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            &mut token,
        ))?;
        // TokenUser 的结构需要指针对齐，不能把 Vec<u8> 假定为 TOKEN_USER 对齐。
        let result = (|| {
            let mut required = 0;
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut required);
            if required == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut buffer = vec![0usize; (required as usize).div_ceil(size_of::<usize>())];
            check(GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                required,
                &mut required,
            ))?;
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            sid_string(user.User.Sid)
        })();
        CloseHandle(token);
        result
    }
}
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::other("路径包含 NUL"));
    }
    value.push(0);
    Ok(value)
}
pub fn create(path: &Path) -> io::Result<()> {
    let sid = current_sid()?;
    // P 禁止继承父目录权限；OICI 使所有候选文件仅继承当前用户的完全访问权。
    let descriptor: Vec<u16> = format!("D:P(A;OICI;FA;;;{sid})\0").encode_utf16().collect();
    let path_wide = wide(path)?;
    unsafe {
        let mut pointer = null_mut();
        check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            1,
            &mut pointer,
            null_mut(),
        ))?;
        let _allocated = Local(pointer);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: pointer,
            bInheritHandle: 0,
        };
        check(CreateDirectoryW(path_wide.as_ptr(), &attributes))?;
    }
    if let Err(error) = verify(path, true) {
        let _ = std::fs::remove_dir(path);
        return Err(error);
    }
    Ok(())
}
pub fn verify(path: &Path, protected: bool) -> io::Result<()> {
    let expected = current_sid()?;
    let path_wide = wide(path)?;
    unsafe {
        let mut dacl = null_mut();
        let mut descriptor = null_mut();
        let error = GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        );
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        let _allocated = Local(descriptor);
        let mut control = 0;
        let mut revision = 0;
        check(GetSecurityDescriptorControl(
            descriptor,
            &mut control,
            &mut revision,
        ))?;
        if dacl.is_null()
            || (*dacl).AceCount != 1
            || (protected && control & SE_DACL_PROTECTED == 0)
        {
            return Err(io::Error::other("密钥路径 DACL 未限制为当前用户"));
        }
        let mut ace = null_mut();
        check(GetAce(dacl, 0, &mut ace))?;
        let allowed = &*ace.cast::<ACCESS_ALLOWED_ACE>();
        // ACCESS_ALLOWED_ACE_TYPE = 0。SidStart 是变长 SID 的首字段而非指针。
        if allowed.Header.AceType != 0
            || allowed.Mask & 0x001f01ff != 0x001f01ff
            || sid_string(std::ptr::addr_of!(allowed.SidStart).cast_mut().cast())? != expected
        {
            return Err(io::Error::other("密钥路径 ACL 验证失败"));
        }
    }
    Ok(())
}
