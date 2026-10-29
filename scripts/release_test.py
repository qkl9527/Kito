"""发布链路的离线测试；所有 GitHub 写操作均由内存 fake 替代。"""

from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import release


TAG = "v0.1.0"
COMMIT = "a" * 40


class FakeGitHub:
    def __init__(self, assets, existing=False, fail_upload=False, moved=False):
        self.assets = assets
        self.existing = existing
        self.fail_upload = fail_upload
        self.moved = moved
        self.calls = []
        self.tag = TAG

    def request(self, method, path, payload=None, asset=None, missing_ok=False):
        self.calls.append((method, path, payload))
        if path.startswith("/releases/tags/"):
            return {"id": 1} if self.existing else None
        if path.startswith("/releases?"):
            return []
        if path.startswith("/commits/"):
            return {"sha": "b" * 40 if self.moved else COMMIT}
        if method == "POST" and path == "/releases":
            self.tag = payload["tag_name"]
            return {"id": 1, "upload_url": "https://uploads.github.com/repos/a/b/releases/1/assets{?name,label}"}
        if asset:
            if self.fail_upload:
                raise RuntimeError("模拟上传失败")
            return {"state": "uploaded", "size": asset.stat().st_size, "name": asset.name}
        if method == "GET" and path == "/releases/1":
            return {"draft": True, "tag_name": self.tag, "assets": [
                {"name": item.name, "size": item.stat().st_size, "state": "uploaded"}
                for item in self.assets
            ]}
        if method == "PATCH":
            return {"draft": False, "html_url": "https://github.com/a/b/releases/tag/v0.1.0"}
        raise AssertionError(f"意外请求：{method} {path}")


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="kito-release-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifests()

    def manifests(self, manifest="0.1.0", locked="0.1.0"):
        (self.root / "Cargo.toml").write_text(
            f'[package]\nname = "kito"\nversion = "{manifest}"\n', encoding="utf-8"
        )
        (self.root / "Cargo.lock").write_text(
            f'[[package]]\nname = "kito"\nversion = "{locked}"\n', encoding="utf-8"
        )

    def artifacts(self):
        artifacts = self.root / "artifacts"
        artifacts.mkdir()
        for target in release.TARGETS:
            directory = artifacts / target
            directory.mkdir()
            archive = directory / release.archive_name(TAG, target)
            archive.write_bytes(target.encode())
            (directory / f"{archive.name}.sha256").write_text(
                f"{release.checksum(archive)}  {archive.name}\n", encoding="utf-8"
            )
        return artifacts

    def test_semver_and_prerelease(self):
        for tag, prerelease in [(TAG, False), ("v1.2.3-rc.1", True), ("v1.2.3+build.01", False)]:
            with self.subTest(tag=tag):
                self.assertEqual(release.tag_version(tag), (tag[1:], prerelease))
        for tag in ["0.1.0", "v1.2", "v01.2.3", "v1.2.3-01", "v1.2.3-", "v1.2.3\n", "v1.2.3/evil"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.tag_version(tag)

    def test_all_versions_must_match(self):
        self.assertEqual(release.validate_version(TAG, self.root), ("0.1.0", False))
        for manifest, locked in [("0.2.0", "0.1.0"), ("0.1.0", "0.2.0")]:
            self.manifests(manifest, locked)
            with self.assertRaises(ValueError):
                release.validate_version(TAG, self.root)

    def test_package_formats_include_executable_docs_and_checksum(self):
        (self.root / "README.md").write_text("Kito", encoding="utf-8")
        license_text = "MIT License\n\nCopyright (c) 2026 Kito test fixture\n"
        (self.root / "LICENSE").write_bytes(license_text.encode("utf-8"))
        docs = self.root / "docs"
        docs.mkdir()
        (docs / "releasing.md").write_text("发布说明", encoding="utf-8")
        for target in ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"]:
            binary_name = "kito.exe" if target.endswith("windows-msvc") else "kito"
            binary = self.root / "target" / target / "release" / binary_name
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"binary")
            binary.chmod(0o755)
            output = self.root / target
            with patch("release.subprocess.check_output", side_effect=[f"host: {target}", "kito 0.1.0"]), patch("release.subprocess.run"):
                release.package(TAG, target, output, self.root)
            archive = output / release.archive_name(TAG, target)
            top = f"kito-{TAG}-{target}"
            if archive.suffix == ".zip":
                with zipfile.ZipFile(archive) as bundle:
                    self.assertEqual(bundle.read(f"{top}/{binary_name}"), b"binary")
                    self.assertIn(f"{top}/docs/releasing.md", bundle.namelist())
                    self.assertEqual(bundle.read(f"{top}/LICENSE"), license_text.encode())
            else:
                with tarfile.open(archive) as bundle:
                    self.assertEqual(bundle.extractfile(f"{top}/{binary_name}").read(), b"binary")
                    self.assertTrue(bundle.getmember(f"{top}/{binary_name}").mode & 0o111)
                    self.assertEqual(bundle.extractfile(f"{top}/LICENSE").read(), license_text.encode())
            self.assertEqual((output / f"{archive.name}.sha256").read_text(), f"{release.checksum(archive)}  {archive.name}\n")

    def test_package_rejects_wrong_native_host_or_binary_version(self):
        for outputs in [["host: wrong"], ["host: aarch64-apple-darwin", "kito 9.9.9"]]:
            with patch("release.subprocess.check_output", side_effect=outputs), self.assertRaises(ValueError):
                release.package(TAG, "aarch64-apple-darwin", self.root / "dist", self.root)
            self.assertFalse((self.root / "dist").exists())

    def test_assemble_requires_complete_verified_set(self):
        artifacts = self.artifacts()
        assets = release.assemble(TAG, artifacts, self.root / "dist")
        self.assertEqual(len(assets), 6)
        manifest = (self.root / "dist" / "SHA256SUMS").read_text()
        self.assertEqual(len(manifest.splitlines()), 5)
        with self.assertRaises(ValueError):
            release.assemble(TAG, artifacts, self.root / "dist")
        archive = next(path for path in artifacts.rglob("*") if path.is_file() and path.suffix != ".sha256")
        archive.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "SHA256"):
            release.assemble(TAG, artifacts, self.root / "bad-dist")
        self.assertFalse((self.root / "bad-dist").exists())
        archive.unlink()
        with self.assertRaisesRegex(ValueError, "缺失"):
            release.assemble(TAG, artifacts, self.root / "missing-dist")

    def test_assemble_rejects_duplicate_asset_names(self):
        artifacts = self.artifacts()
        source = next(path for path in artifacts.rglob("*") if path.is_file())
        (artifacts / source.name).write_bytes(source.read_bytes())
        with self.assertRaises(ValueError):
            release.assemble(TAG, artifacts, self.root / "dist")

    def test_existing_release_or_moved_tag_never_writes(self):
        for kwargs in [{"existing": True}, {"moved": True}]:
            api = FakeGitHub([], **kwargs)
            with self.assertRaises(ValueError):
                release.publish_release(api, TAG, COMMIT, [])
            self.assertTrue(all(call[0] == "GET" for call in api.calls))

    def test_upload_failure_does_not_publish_draft(self):
        assets = release.assemble(TAG, self.artifacts(), self.root / "dist")
        api = FakeGitHub(assets, fail_upload=True)
        with self.assertRaises(RuntimeError):
            release.publish_release(api, TAG, COMMIT, assets)
        self.assertFalse(any(call[0] == "PATCH" for call in api.calls))
        create = next(call[2] for call in api.calls if call[0:2] == ("POST", "/releases"))
        self.assertTrue(create["draft"])

    def test_draft_on_later_page_blocks_release_creation(self):
        api = FakeGitHub([])
        original = api.request

        def request(method, path, *args, **kwargs):
            if path == "/releases?per_page=100&page=1":
                return [{"tag_name": f"v9.0.{number}", "draft": False} for number in range(100)]
            if path == "/releases?per_page=100&page=2":
                return [{"tag_name": TAG, "draft": True}]
            return original(method, path, *args, **kwargs)

        with patch.object(api, "request", side_effect=request), self.assertRaisesRegex(ValueError, "已有 Release"):
            release.publish_release(api, TAG, COMMIT, [])
        self.assertTrue(all(call[0] == "GET" for call in api.calls))

    def test_publish_only_after_all_uploads_and_marks_prerelease(self):
        assets = release.assemble(TAG, self.artifacts(), self.root / "dist")
        api = FakeGitHub(assets)
        release.publish_release(api, TAG, COMMIT, assets)
        self.assertEqual(len([call for call in api.calls if call[1].startswith("https://uploads.")]), 6)
        self.assertEqual(api.calls[-1], ("PATCH", "/releases/1", {"draft": False, "prerelease": False}))
        api = FakeGitHub(assets)
        release.publish_release(api, "v0.1.0-rc.1", COMMIT, assets)
        self.assertTrue(api.calls[-1][2]["prerelease"])

    def test_github_errors_are_not_treated_as_missing_release(self):
        api = release.GitHub("owner/repository", "test-token")
        for code in [401, 403, 500]:
            error = release.urllib.error.HTTPError("https://api.github.com", code, "error", {}, None)
            with patch("release.urllib.request.urlopen", side_effect=error), self.assertRaises(RuntimeError):
                api.request("GET", "/releases/tags/v0.1.0", missing_ok=True)
        error = release.urllib.error.HTTPError("https://api.github.com", 404, "error", {}, None)
        with patch("release.urllib.request.urlopen", side_effect=error):
            self.assertIsNone(api.request("GET", "/releases/tags/v0.1.0", missing_ok=True))


if __name__ == "__main__":
    unittest.main()
