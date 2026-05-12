# Отчёт по лабораторной работе №1

**Дисциплина:** Современные технологии программирования
**Тема ЛР:** Разработка серверных веб-приложений на языке Rust с использованием фреймворка Rocket и интеграцией с нейросетевыми API
**Выбранная тема проекта:** «Умный конспектор» (тема №1 из раздела 3.2)

**Студент:** \<ФИО\>
**Группа:** \<группа\>
**Год:** 2026

---

## 1. Цель работы

Освоить разработку серверного веб-приложения на Rust с фреймворком Rocket
и интеграцией со сторонним AI-сервисом (GigaChat). В ходе работы — изучить
построение REST API, разделение кода на модули, применение трейтов для
абстракции сервисов, написание интеграционных тестов и работа в mock-режиме.

В качестве собственной темы выбран «Умный конспектор» — сервис, который
принимает на вход произвольный текст и возвращает краткую выжимку плюс
список ключевых терминов.

## 2. Анализ демонстрационного проекта

Перед реализацией собственной темы выполнен разбор демонстрационного
проекта `rust-gigachat-demo`. Структура проекта:

```
src/
├── main.rs        — точка входа, сборка Rocket и DI
├── config/        — загрузка config.toml + переопределения через env
├── models/        — DTO: AskRequest, AskResponse, HealthResponse, ErrorResponse
├── handlers/      — HTTP-обработчики: /, /health, /ask + catchers
└── services/      — трейт AiService и две реализации:
                     - GigaChatService (реальный API через gigalib)
                     - MockAiService (захардкоженные ответы)
```

Ключевая идея архитектуры — **трейт `AiService` как абстракция над «движком»**.
Handler `ask` не знает про GigaChat: он получает `&State<Box<dyn AiService>>`
и вызывает `ai_service.ask(...)`. Конкретная реализация выбирается
`AiServiceFactory` на этапе старта приложения в зависимости от наличия
переменной `GIGACHAT_TOKEN` и поля `gigachat.enabled` в `config.toml`.

Это удобно для расширения: добавление новой темы не требует изменений в
существующих сервисах — достаточно использовать тот же `ai_service.ask(prompt)`
с подходящим промптом.

После разбора кода демо-проект был запущен в mock-режиме. Прогон
`examples/test_api.sh` и `cargo run --example simple_client` отработал
корректно, `cargo test` показал 26 проходящих тестов.

## 3. Архитектура «Умного конспектора»

Новый эндпоинт `POST /summarize` принимает JSON:

```json
{ "text": "...", "max_terms": 5 }
```

и возвращает:

```json
{
  "summary": "Краткая выжимка (1–2 предложения).",
  "key_terms": ["термин1", "термин2", "термин3"],
  "source": "mock ai service"
}
```

Поле `max_terms` опциональное, значение по умолчанию — 5.

Поток данных:

```
POST /summarize  {"text": "...", "max_terms": N}
        |
        v
handlers::summarize
        |
        | формирует промпт:
        |   "Сделай краткий конспект следующего текста
        |    и перечисли до N ключевых терминов. Текст: ..."
        v
ai_service.ask(prompt)
        |
        v
MockAiService:                          GigaChatService:
узнаёт префикс «Сделай краткий          реальный AI отвечает свободным
конспект» и возвращает строку           текстом
с маркерами:
  <<<SUMMARY>>>
  Первое предложение текста.
  <<<TERMS>>>
  term1
  term2
        |
        v
handlers::parse_summary_payload(answer)
        |
        | если маркеры есть — раскладывает на summary и key_terms;
        | если нет — кладёт весь ответ в summary, key_terms = []
        v
SummarizeResponse → Rocket сериализует → JSON клиенту
```

В работе было принято решение **не менять трейт `AiService`**. Альтернативный
подход — добавить в трейт метод `summarize()` — потребовал бы модификации
обеих реализаций (`GigaChatService`, `MockAiService`) и фабрики. Вместо этого
зафиксирован договор о формате ответа через два маркера-разделителя:
`<<<SUMMARY>>>` и `<<<TERMS>>>`. Mock-сервис строго соблюдает этот формат,
а handler выполняет разбор. Если AI-сервис вернул ответ без маркеров
(например, реальный GigaChat не выполнил инструкцию формата), handler не
завершается с ошибкой: весь текст ответа помещается в поле `summary`, а
`key_terms` остаётся пустым. Такое поведение обеспечивает безопасный фолбэк.

Логика mock-расчёта:
- **summary** — первое предложение текста (до точки, знака восклицания или вопроса).
- **key_terms** — топ-N слов длиной от пяти символов, отсортированных по
  частоте; при равной частоте — по алфавиту, чтобы результат был
  детерминирован.

Для реального GigaChat эта простая эвристика, разумеется, не выполняется —
там работает полноценная модель. Для учебного mock-режима поведение
выглядит правдоподобно и достаточно для отладки клиентского кода.

## 4. Ключевые фрагменты кода

### 4.1. Модели (`src/models/mod.rs`)

```rust
#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct SummarizeRequest {
    pub text: String,

    #[serde(default)]
    pub max_terms: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct SummarizeResponse {
    pub summary: String,
    pub key_terms: Vec<String>,
    pub source: String,
}
```

Используется тот же подход к derive-атрибутам, что и для существующих
`AskRequest`/`AskResponse`: для Request — только `Deserialize`, для
Response — только `Serialize`. Атрибут `#[serde(default)]` на `max_terms`
позволяет клиенту опустить это поле — Serde подставит `None`.

### 4.2. Handler (`src/handlers/mod.rs`)

```rust
#[post("/summarize", format = "json", data = "<request>")]
pub async fn summarize(
    request: Json<SummarizeRequest>,
    ai_service: &State<Box<dyn AiService>>,
) -> Result<Json<SummarizeResponse>, Json<ErrorResponse>> {
    let text = request.text.trim();
    info!("Received summarize request ({} chars)", text.len());

    if text.is_empty() {
        return Err(Json(ErrorResponse::with_code(
            "Text cannot be empty",
            "EMPTY_TEXT",
        )));
    }

    let max_terms = request.max_terms.unwrap_or(DEFAULT_MAX_KEY_TERMS);

    let prompt = format!(
        "Сделай краткий конспект следующего текста (1-2 предложения) и затем \
         перечисли до {max_terms} ключевых терминов через запятую. Текст:\n\n{text}"
    );

    match ai_service.ask(&prompt).await {
        Ok(answer) => {
            let (summary, key_terms) = parse_summary_payload(&answer, max_terms);
            Ok(Json(SummarizeResponse {
                summary,
                key_terms,
                source: ai_service.name().to_lowercase(),
            }))
        }
        Err(e) => Err(Json(ErrorResponse::with_code(
            format!("Failed to summarize: {e}"),
            "AI_SERVICE_ERROR",
        ))),
    }
}
```

Парсер ответа вынесен в отдельную функцию для удобства тестирования и
читаемости основного handler:

```rust
fn parse_summary_payload(answer: &str, max_terms: usize) -> (String, Vec<String>) {
    let Some(summary_start) = answer.find(MOCK_SUMMARIZE_SUMMARY_TAG) else {
        return (answer.trim().to_string(), Vec::new());
    };

    let after_summary_tag = &answer[summary_start + MOCK_SUMMARIZE_SUMMARY_TAG.len()..];
    let (summary_block, terms_block) =
        if let Some(terms_idx) = after_summary_tag.find(MOCK_SUMMARIZE_TERMS_TAG) {
            let summary = &after_summary_tag[..terms_idx];
            let terms = &after_summary_tag[terms_idx + MOCK_SUMMARIZE_TERMS_TAG.len()..];
            (summary, terms)
        } else {
            (after_summary_tag, "")
        };

    let summary = summary_block.trim().to_string();
    let key_terms: Vec<String> = terms_block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(max_terms)
        .map(str::to_string)
        .collect();

    (summary, key_terms)
}
```

### 4.3. Mock-логика (`src/services/mod.rs`)

В метод `MockAiService::ask` добавлена одна ветка в начале: если промпт
начинается с известного префикса, сервис возвращает специальный
структурированный payload:

```rust
async fn ask(&self, question: &str) -> Result<String, AiServiceError> {
    if question.starts_with(SUMMARIZE_PROMPT_PREFIX) {
        return Ok(build_mock_summary_payload(question));
    }

    // Дальше — прежняя логика mock-ответов по темам Rust / Rocket / async / ...
}
```

Функция-строитель payload:

```rust
fn build_mock_summary_payload(prompt: &str) -> String {
    let max_terms = extract_max_terms(prompt).unwrap_or(5);
    let text = extract_source_text(prompt);

    let summary = first_sentence(text).unwrap_or_else(|| text.trim().to_string());
    let terms = top_frequent_terms(text, max_terms);

    let mut out = String::new();
    out.push_str(MOCK_SUMMARIZE_SUMMARY_TAG);
    out.push('\n');
    out.push_str(&summary);
    out.push('\n');
    out.push_str(MOCK_SUMMARIZE_TERMS_TAG);
    out.push('\n');
    for t in terms {
        out.push_str(&t);
        out.push('\n');
    }
    out
}
```

Вспомогательные функции `extract_max_terms`, `extract_source_text`,
`first_sentence`, `top_frequent_terms` вынесены отдельно. В частности,
`top_frequent_terms` использует `HashMap` для подсчёта частот слов и
сортировку с тай-брейком по алфавиту, что обеспечивает стабильность
вывода при равной частоте.

### 4.4. Регистрация маршрута (`src/main.rs`)

Маршрут `summarize` подключён к Rocket рядом с остальными:

```rust
.mount("/", routes![index, health, ask, summarize, cors_preflight])
```

Функция `summarize` также добавлена в импорты модуля `handlers`.

## 5. Тестирование

### 5.1. Интеграционные тесты

В файл `tests/integration_test.rs` добавлено пять тестов, использующих
`rocket::local::blocking::Client` с принудительно подменённым на
`MockAiService` сервисом:

| Тест | Что проверяет |
|---|---|
| `test_summarize_happy_path` | связный текст из 4 предложений: статус 200, summary начинается с «Rust» и заканчивается точкой, `key_terms` непуст и содержит не более 5 элементов |
| `test_summarize_respects_max_terms` | при `max_terms = 2` ответ содержит не более двух терминов |
| `test_summarize_empty_text` | пустой `text` приводит к ответу с кодом `EMPTY_TEXT` |
| `test_summarize_missing_text_field` | JSON без поля `text` возвращает HTTP 422 Unprocessable Entity |
| `test_summarize_zero_max_terms` | при `max_terms = 0` поле `key_terms` пусто, `summary` непуст |

Финальный прогон `cargo test`:

```
running 9 tests   …  9 passed   (lib)
running 10 tests  … 10 passed   (unittests src/main.rs)
running 12 tests  … 12 passed   (integration_test, из них 5 — для /summarize)
running 13 tests  …  3 passed; 10 ignored   (doc-tests)

Итого: 34 passed; 0 failed.
```

### 5.2. Ручная проверка через curl

После запуска сервера командой `cargo run` (mock-режим) была проведена
проверка нескольких сценариев.

**Связный текст:**
```
$ curl -s -X POST http://127.0.0.1:8000/summarize \
    -H "Content-Type: application/json" \
    -d '{"text":"Rust — современный системный язык программирования.
       Он обеспечивает безопасность памяти без сборщика мусора.
       Rust популярен для веб-серверов, встраиваемых систем и
       инструментов командной строки. Сообщество Rust большое и
       активное."}'

{
  "summary": "Rust — современный системный язык программирования.",
  "key_terms": ["активное","безопасность","большое","веб-серверов","встраиваемых"],
  "source": "mock ai service"
}
```

**С ограничением `max_terms = 3`:**
```
$ curl -s -X POST http://127.0.0.1:8000/summarize \
    -H "Content-Type: application/json" \
    -d '{"text":"Rocket — это веб-фреймворк для языка Rust. Rocket делает
       разработку приложений быстрой и безопасной. Rocket поддерживает
       асинхронность, JSON и кастомные fairings.","max_terms":3}'

{
  "summary": "Rocket — это веб-фреймворк для языка Rust.",
  "key_terms": ["rocket","fairings","асинхронность"],
  "source": "mock ai service"
}
```

**Граничные случаи:**

```
$ curl -s -X POST http://127.0.0.1:8000/summarize \
    -H "Content-Type: application/json" -d '{"text":""}'
{"error":"Text cannot be empty","code":"EMPTY_TEXT"}

$ curl -s -X POST http://127.0.0.1:8000/summarize \
    -H "Content-Type: application/json" -d '{"max_terms":3}'
{"error":"Invalid request format. Check your JSON.","code":"INVALID_REQUEST"}
```

В обоих случаях handler возвращает осмысленную JSON-ошибку с
машиночитаемым кодом.

## 6. Выводы

В ходе выполнения работы получены практические навыки и закреплены
следующие концепции:

- Маршрутизация Rocket через атрибутные макросы
  (`#[post("/path", format = "json", data = "<…>")]`) и автоматическая
  десериализация JSON-тела через `serde` при использовании `Json<T>`.
- Назначение абстракции через трейт `AiService` и тип `Box<dyn AiService>`:
  добавление нового эндпоинта не потребовало изменений ни в
  `GigaChatService`, ни в `MockAiService` на уровне публичного API —
  расширение реализовано через **формат данных**, а не через расширение трейта.
- Применение паттерна «безопасный фолбэк»: если AI-сервис возвращает ответ
  без ожидаемых маркеров, клиент всё равно получает корректный, непустой
  результат — поле `summary` заполняется текстом ответа целиком, поле
  `key_terms` остаётся пустым.
- Интеграционное тестирование Rocket-приложений без подъёма реального
  процесса — через `rocket::local::blocking::Client`. Этот подход
  обеспечивает изоляцию тестов, скорость и детерминизм.
- Соблюдение принятого в проекте формата коммитов с issue-ID, типом и
  приоритетом (`[<id>] <type>(P#): <title>` + описание + список изменённых
  файлов).

Возможные направления дальнейшего развития проекта (выходят за рамки ЛР1):

- Подключение реального GigaChat и проверка, насколько модель соблюдает
  инструкции формата ответа с маркерами `<<<SUMMARY>>>`/`<<<TERMS>>>`;
  тестирование работы фолбэка на реальных кейсах.
- Улучшение алгоритма выбора ключевых терминов — фильтрация стоп-слов
  («это», «как», «для», «при» и т.д.), которые при длинном тексте
  ошибочно попадают в результат.
- Добавление параметра «максимальная длина выжимки в символах».

В целом архитектура учебного проекта оказалась удобной для расширения:
аккуратное разделение модулей и трейт-объектная абстракция AI-сервиса
позволили реализовать новую тему без вмешательства в существующий
фундамент.
