# Testnet Bridge

Простая реализация LayerZero Testnet Bridge на языке Rust в целях изучения нового языка.

На текущий момент сделан только мост Arbitrum -> Goerli.

# Установка

## Установка Rust и Cargo

Установите необходимое ПО по ссылке: https://doc.rust-lang.org/cargo/getting-started/installation.html

## Установка скрипта

```
git clone https://github.com/hodlmod/testnet_bridge.git
cd testnet_bridge
cargo build --release
```

# Настройка

## Приватные ключи

Приватники через перенос строки вставьте в файл privates.txt

## Задержки и рандомная отправка эфира

Все настройки производятся в файле src/main.rs

RANDOM_MIN, RANDOM_MAX - минимальные и максимальные значения задержки

RANDOM_ETH_MIN, RANDOM_ETH_MAX - минимальные и максимальные значения эфира

# Запуск

```
cargo run --release
```
