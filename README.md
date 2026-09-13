# 靈明输入法构词程序

使用方法：将灵明的拆分与词库文件放在程序运行目录下，运行程序即可。

```bash
$ ls .     
Cargo.lock
Cargo.toml
README.md
src/
target/
yuling_chaifen.dict.yaml    # 陆拆数据
yuling_chaifen_tw.dict.yaml # 台拆数据
yuling_sc.words.dict.yaml # 简体词库
yuling_tc.words.dict.yaml # 繁体词库

# 确保已有以上文件后，运行程序即可
$ cargo run
```

如果想要计算词库的重码性能，运行 `check_dupes.sh` 脚本：

```bash
$ ./check_dupes.sh
```
