fn main() {
    // 开启 embed-cloudflared 时，把二进制路径通过环境变量交给 include_bytes!。
    // 找不到就直接构建失败——宁可不出包，也不要产出一个「装了却没有引擎」的版本。
    #[cfg(feature = "embed-cloudflared")]
    {
        let triple = std::env::var("TARGET").expect("cargo 必定提供 TARGET");
        let ext = if triple.contains("windows") { ".exe" } else { "" };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("cloudflared-{triple}{ext}"));

        if !path.is_file() {
            panic!(
                "未找到内置 cloudflared：{}\n请先运行 `node scripts/fetch-cloudflared.mjs`",
                path.display()
            );
        }

        println!("cargo:rustc-env=EASY_PORT_CLOUDFLARED={}", path.display());
        println!("cargo:rerun-if-changed={}", path.display());
    }

    tauri_build::build()
}
