# CSS Parser 编译指南

## 目录

- [环境要求](#环境要求)
- [本地编译](#本地编译)
  - [Linux (x86_64)](#linux-x86_64)
  - [Linux (ARM64)](#linux-arm64-aarch64)
  - [macOS](#macos)
  - [Windows](#windows)
- [跨平台交叉编译](#跨平台交叉编译)
  - [x86_64 Linux 交叉编译 ARM64](#x86_64-linux-交叉编译-arm64)
  - [Linux 交叉编译 Windows](#linux-交叉编译-windows)
- [Manylinux (glibc 2.17+)](#manylinux-glibc-217)
- [GitHub Actions 自动构建](#github-actions-自动构建)
- [常见问题](#常见问题)

---

## 环境要求

### 必需

- **Rust** (最新稳定版)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustc --version  # >= 1.70
  ```

- **Python** 3.7+
  ```bash
  python3 --version
  ```

- **maturin** (PyO3 构建工具)
  ```bash
  pip install maturin
  ```

### 平台特定

| 平台 | 额外依赖 |
|------|---------|
| Linux | `gcc`, `clang`, `cmake` (部分依赖需要) |
| macOS | Xcode Command Line Tools |
| Windows | Visual Studio Build Tools |

---

## 本地编译

### Linux (x86_64)

```bash
# 1. 进入项目目录
cd css_parser

# 2. 安装 maturin
pip install maturin

# 3. 编译并安装
maturin develop

# 或只构建 wheel
maturin build
```

### Linux ARM64 (aarch64)

```bash
# 1. 安装交叉编译工具链 (如果不在 ARM64 上)
# 在 x86_64 上:
sudo apt install gcc-aarch64-linux-gnu

# 2. 在 ARM64 本机上直接编译
cd css_parser
pip install maturin
maturin develop

# 或交叉编译
maturin build --target aarch64-unknown-linux-gnu
```

### macOS

```bash
# 1. 安装 Xcode Command Line Tools
xcode-select --install

# 2. 安装 maturin
pip install maturin

# 3. 编译
maturin develop

# Intel Mac
maturin build --target x86_64-apple-darwin

# Apple Silicon Mac
maturin build --target aarch64-apple-darwin

# 通用二进制 (Intel + Apple Silicon)
maturin build --target universal2-apple-darwin
```

### Windows

```powershell
# 1. 安装 Rust (已在 Windows 上)
# 下载: https://rustup.rs

# 2. 安装 Visual Studio Build Tools
# 下载: https://visualstudio.microsoft.com/downloads/
# 选择: "C++ 生成工具"

# 3. 安装 maturin
pip install maturin

# 4. 编译
maturin build --release

# x64
maturin build --release --target x64-pc-windows-msvc

# ARM64
maturin build --release --target aarch64-pc-windows-msvc
```

---

## 跨平台交叉编译

### x86_64 Linux 交叉编译 ARM64

#### 方法 1: 使用 maturin-action (推荐)

编辑 `pyproject.toml`:

```toml
[tool.maturin]
targets = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
]
manylinux = "2014"
```

使用 GitHub Actions 交叉编译（见下文）

#### 方法 2: 本地交叉编译

```bash
# 安装 ARM64 工具链
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu

# 创建 .cargo/config.toml
mkdir -p .cargo
cat > .cargo/config.toml << 'EOF'
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"

[build]
rustflags = ["-C", "target-feature=-crt-static"]
EOF

# 编译
maturin build --release --target aarch64-unknown-linux-gnu
```

### Linux 交叉编译 Windows

```bash
# 安装 MinGW-w64
sudo apt install mingw-w64

# 创建 .cargo/config.toml
cat > .cargo/config.toml << 'EOF'
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"

[target.i686-pc-windows-gnu]
linker = "i686-w64-mingw32-gcc"
EOF

# 编译
maturin build --release --target x86_64-pc-windows-gnu
```

---

## Manylinux (glibc 2.17+)

项目已配置 `manylinux = "2014"` (CentOS 7, glibc 2.17+)

### 本地构建 Manylinux wheel

```bash
# 使用 Docker
docker run --rm -v $(pwd):/io quay.io/pypa/manylinux2014_x86_64 \
    bash -c "pip install maturin && cd /io && maturin build --release"

# 或使用 maturin manylinux-image
maturin build --release --manylinux 2014
```

### 支持的 glibc 版本

| Manylinux 版本 | glibc 最低版本 | 支持的 Linux 发行版 |
|----------------|---------------|-------------------|
| manylinux_2_17 | 2.17 | CentOS 7, Amazon Linux 2 |
| manylinux_2_24 | 2.24 | Debian 9+ |
| manylinux_2_27 | 2.27 | Ubuntu 18.04, Debian 10 |
| manylinux_2_31 | 2.31 | Ubuntu 20.04 |

---

## GitHub Actions 自动构建

项目已配置 `.github/workflows/release.yml`，推送 tag 时自动构建所有平台。

### 手动触发构建

```bash
# 创建 tag
git tag v0.1.0
git push origin v0.1.0
```

### 修改构建目标

编辑 `.github/workflows/release.yml`:

```yaml
jobs:
  build_linux:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu      # x86_64
          - aarch64-unknown-linux-gnu     # ARM64

  build_windows:
    strategy:
      matrix:
        target:
          - x64                            # Windows x64
          - x86                            # Windows x86 (可选)

  build_macos:
    strategy:
      matrix:
        target:
          - x86_64-apple-darwin           # Intel Mac
          - aarch64-apple-darwin          # Apple Silicon Mac
```

### 构建产物位置

```
dist/
├── css_parser-0.1.0-cp312-cp312-manylinux_2_17_x86_64.whl
├── css_parser-0.1.0-cp312-cp312-manylinux_2_17_aarch64.whl
├── css_parser-0.1.0-cp312-cp312-win_amd64.whl
├── css_parser-0.1.0-cp312-cp312-macosx_x86_64.whl
└── css_parser-0.1.0-cp312-cp312-macosx_arm64.whl
```

---

## 常见问题

### Q: 编译报错 "error: linker `cc` not found"

```bash
# Debian/Ubuntu
sudo apt install build-essential

# CentOS/RHEL
sudo yum groupinstall "Development Tools"
```

### Q: ARM64 交叉编译失败

确保安装正确的工具链:

```bash
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
```

### Q: Windows 编译报错 "LINK : fatal error LNK1181: cannot open input file"

确保安装 Visual Studio Build Tools，并选择 "C++ 生成工具"。

### Q: manylinux 构建失败

使用 Docker 镜像:

```bash
docker pull quay.io/pypa/manylinux2014_x86_64
```

### Q: 如何查看支持的 Python 版本?

编辑 `pyproject.toml`:

```toml
[project]
requires-python = ">=3.8"
```

### Q: maturin build 生成的 wheel 在哪里?

默认在 `target/wheels/` 目录。

使用 `--out dist` 可以指定输出目录:

```bash
maturin build --out dist
```

---

## 快速参考

| 命令 | 说明 |
|------|------|
| `maturin develop` | 编译并安装到当前 Python 环境 |
| `maturin build` | 构建 wheel 到 `target/wheels/` |
| `maturin build --release` | Release 模式优化构建 |
| `maturin build --target aarch64-unknown-linux-gnu` | ARM64 交叉编译 |
| `maturin build --manylinux 2014` | Manylinux 兼容构建 |
| `maturin publish` | 发布到 PyPI |

---

## 版本历史

| 版本 | 日期 | 变化 |
|------|------|------|
| 0.1.0 | 2024 | 初始版本，支持 x86_64, ARM64, Windows, macOS |
