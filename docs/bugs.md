# Ловушки единиц измерения

## sysinfo 0.30: units

`Process::memory()` в 0.30+ возвращает БАЙТЫ (до 0.30 — КИЛОБАЙТЫ).
Делить нужно на 1024² (`crate::gpu::bytes_to_mb`), а не на 1024.
Регрессионный тест: `gpu::tests::memory_bytes_to_mb_is_1024_based`.
Проверять при апгрейде sysinfo > 0.30.

## 2026-10-02 - Updater end-to-end success
All C13-series (C13a retry download, C13b watchdog, C13c lock file),
C12 (disable button), C14 (updater mutex), C15 (restart after update)
closed. Full pipeline works in live test beta.6+fix -> beta.7.
See logs/updater.log for step-by-step confirmation.
Committed: 2fba0a1 (mutex race fix), e8e3211 (beta.7 release).

## C15: перезапуск после успеха не происходит (2026-10-01)

Живой тест beta.2 → beta.3: файлы заменены (ok=true), но старый процесс
не вышел, новый упёрся в C5-mutex и молча завершился (FindWindowW нашёл
чёрное окно → ShowWindow → exit(0), main.rs:407-419).

Причины в коде (все три нужны для бага):
1. `start_update_install` (app.rs:1408-1443) ставит quit_requested=true,
   но процесс НЕ завершает — в отличие от AppCmd::Quit (app.rs:2614-2616),
   который шлёт ViewportCommand::Close. Успешный путь quit_requested
   никто не сбрасывает (app.rs:2866 — только откат при !ok).
2. Updater спавнит новый процесс ДО записи отчёта (update.rs:1224 раньше
   1240) и не ждёт смерти старого: явного kill нет, wait_for_exit ждёт
   только файл-флаг + 600 мс, не смерть PID.
3. C5-mutex (main.rs:358, тест single_instance_mutex_name_is_stable)
   не различает «второй экземпляр» и «перезапуск апдейтером».

Выбран вариант B: главный процесс сам вызывает process::exit(0), увидев
ok=true (в update() после проверки сторожа); updater ждёт смерть старого
(OpenProcess + WaitForSingleObject по PID из pid-файла рядом с флагом)
и только потом spawn; spawn перенести ПОСЛЕ write_report.
Вариант A (TerminateProcess из updater) отвергнут: грубый kill рискует
незаписанными DB/config; PID всё равно пришлось бы передавать — тот же
механизм, меньше контроля. Вариант C (--spawned-by-updater обходит mutex)
отвергнут: два живых процесса делят sessions.db, старый висит чёрным.

Открытый вопрос (на код не влияет): как замена exe удалась при живом
старом процессе — Windows должен был отклонить copy sharing violation.
Возможно, окно успели закрыть крестиком (при quit_requested=true
CancelClose НЕ ставится, app.rs:2628 — закрытие проходит).
