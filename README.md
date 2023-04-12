# Testnet Bridge

Простая реализация LayerZero Testnet Bridge на языке Rust в целях изучения новго языка.

# Установка

```
git clone https://github.com/hodlmod/testnet_bridge.git
cd testnet_bridge
cargo build --release
```

# Настройка

Приватники через перенос строки вставьте в файл privates.txt
RANDOM_MIN, RANDOM_MAX - минимальные и максимальные значения задержки
RANDOM_ETH_MIN, RANDOM_ETH_MAX - минимальные и максимальные значения эфира

# Запуск

```
cargo run --release
```
