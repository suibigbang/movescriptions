# Movescriptions

[What's move inscriptions](https://github.com/movescriptions/movescriptions)

## Auto Mint #MOVE inscription

The repo provides a command to auto mint MOVE inscription in https://mrc20.fun/ticks/move.
It will mint a inscription in each epoch.

``` shell
cargo build --release
```

```shell
./target/release/movescriptions mint -k sui.key --tick MOVE --tick-address 0xfa6f8ab30f91a3ca6f969d117677fb4f669e08bbeed815071cf38f4d19284199 -f 100000000
```

replace the sui.key file with your file containing your mnemonic words.
