# ONNX Runtime (ort) 编译与链接说明

## 已完成的代码修改（ONNX ops 11→18 / ort 2.x）

- **ort API 适配**  
  - `run(ort::inputs![...]?)` 改为 `run(ort::inputs![...])?`（`inputs!` 返回数组，不能对整体用 `?`）。  
  - 输出改用 `result[0].try_extract_array::<f32>()?`，直接得到 `ndarray::ArrayViewD`，不再用 `try_extract_tensor` + `from_shape`。  
  - `Session::run` 需要 `&mut self`，已用 `RefCell<Session>` 包装并在调用时 `borrow_mut()`，且保证在借用的生命周期内完成对 `result`/`arr` 的使用。

- **tract_onnx 相关**  
  - 在 `yas/Cargo.toml` 中增加了可选 feature `tract_onnx = []`，消除 `unexpected_cfgs` 警告。

## 若出现链接错误 LNK2019 / LNK1120（如 `__std_find_first_of_trivial_pos_1`）

这是 **ort-sys 使用的预编译 ONNX Runtime** 与当前 **MSVC 的 C++ 标准库版本** 不一致导致的：

- 预编译包是用较新的 MSVC（带新 STL 符号）构建的；
- 你本机的 `link.exe` 使用的标准库较旧，没有这些符号，就会报 “unresolved external symbol”。

**推荐做法（任选其一）：**

1. **升级 Visual Studio / Build Tools**  
   安装最新版本（含 MSVC 和 “使用 C++ 的桌面开发”），使本机 STL 与 ort 预编译包一致。然后重新打开终端，执行 `cargo clean && cargo build`。

2. **固定使用较旧的 ort 并重编**  
   在根目录 `Cargo.toml` 的 `[workspace]` 或依赖 ort 的 crate 里把 ort 版本锁到 2.0.0-rc.9，然后：
   - 删除 `Cargo.lock` 或在该 crate 下运行 `cargo update -p ort -p ort-sys` 只更新 ort 相关锁版本；
   - 再执行 `cargo clean && cargo build`。  
   （若该版本的预编译包仍依赖新 STL，则仍需升级 MSVC。）

3. **从源码构建 ONNX Runtime（高级）**  
   查阅 [ort 文档](https://github.com/pyke/ort) 中关于关闭 “download-binaries”、从源码编译 ort-sys 的说明，用你本机 MSVC 和 STL 构建，可避免与预编译包 ABI 不一致。

当前 Rust 侧与 ort 2.x 的编译已通过，剩余的是 **链接阶段** 的 MSVC/STL 环境问题，按上面任一步骤调整环境后即可通过链接。
