# Ловушки единиц измерения

## sysinfo 0.30: units

`Process::memory()` в 0.30+ возвращает БАЙТЫ (до 0.30 — КИЛОБАЙТЫ).
Делить нужно на 1024² (`crate::gpu::bytes_to_mb`), а не на 1024.
Регрессионный тест: `gpu::tests::memory_bytes_to_mb_is_1024_based`.
Проверять при апгрейде sysinfo > 0.30.
