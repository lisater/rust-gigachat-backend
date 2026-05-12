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
и интеграцией со сторонним AI-сервисом (GigaChat). По ходу работы — научиться
строить REST API, разделять код на модули, применять трейты для абстракции
сервисов, писать интеграционные тесты и работать с mock-режимом.

В качестве собственной темы выбран «Умный конспектор» — сервис, который
принимает на вход произвольный текст и возвращает краткую выжимку плюс
список ключевых терминов.

## 2. Анализ демонстрационного проекта

Перед тем как писать что-то своё, я разобрался с тем, что уже есть в репозитории.
Демо-проект `rust-gigachat-demo` устроен так:

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
Handler `ask` ничего не знает про GigaChat: он получает `&State<Box<dyn AiService>>`
и вызывает `ai_service.ask(...)`. Какая именно реализация подставлена — решает
`AiServiceFactory` на этапе старта приложения в зависимости от наличия
переменной `GIGACHAT_TOKEN` и поля `gigachat.enabled` в `config.toml`.

Это удобно: для своей темы я **не трогаю** этот фундамент, а просто пользуюсь
им через тот же `ai_service.ask(prompt)`.

Запустил демо в mock-режиме, прогнал `examples/test_api.sh` и `cargo run --example simple_client` — всё работает. `cargo test` показывает 26 проходящих тестов.

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

Поле `max_terms` опциональное, по умолчанию 5.

Поток данных получается такой:

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

Главное решение, которое я тут принял — **не менять трейт `AiService`**.
Можно было бы добавить в него метод `summarize()`, но тогда пришлось бы
менять и `GigaChatService`, и `MockAiService`, и фабрику. Вместо этого я
договорился о формате ответа через два маркера-разделителя — `<<<SUMMARY>>>`
и `<<<TERMS>>>`. Mock возвращает строго такой формат, а handler умеет его
разобрать. Если AI вернул что-то другое (например, реальный GigaChat
проигнорировал просьбу про маркеры) — handler не падает, а кладёт весь
текст в `summary`, а `key_terms` оставляет пустым. Это безопасный фолбэк.

В этой же логике mock считает выжимку и ключевые термины «как умеет»:
- **summary** — первое предложение текста (до точки, знака восклицания или вопроса).
- **key_terms** — топ-N слов длиной от 5 символов, по частоте; при равной частоте — по алфавиту, чтобы вывод был детерминирован.

Для реальной GigaChat этот код, конечно, работать так же не будет — там
была бы настоящая модель. Но для учебного mock-режима поведение
выглядит правдоподобно.

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

Я следую тому же стилю, что и для существующих `AskRequest`/`AskResponse`:
у Request только `Deserialize`, у Response только `Serialize`. Атрибут
`#[serde(default)]` на `max_terms` позволяет клиенту не передавать это
поле — Serde подставит `None`.

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

Парсер ответа — отдельная функция, чтобы её удобно было тестировать
и чтобы основной handler оставался компактным:

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

В `MockAiService::ask` я добавил одну ветку в самое начало — если промпт
начинается с известного префикса, отдаём специальный формат:

```rust
async fn ask(&self, question: &str) -> Result<String, AiServiceError> {
    if question.starts_with(SUMMARIZE_PROMPT_PREFIX) {
        return Ok(build_mock_summary_payload(question));
    }

    // ... дальше старая логика mock-ответов про Rust/Rocket/async/...
}
```

Сама функция, которая собирает payload:

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

Хелперы `extract_max_terms`, `extract_source_text`, `first_sentence`,
`top_frequent_terms` вынесены отдельно. `top_frequent_terms` использует
`HashMap` для подсчёта и сортировку с тай-брейком по алфавиту, чтобы при
равной частоте вывод был стабильным.

### 4.4. Регистрация маршрута (`src/main.rs`)

```rust
.mount("/", routes![index, health, ask, summarize, cors_preflight])
```

Плюс импорт функции `summarize` рядом с остальными handler’ами.

## 5. Тестирование

### 5.1. Юнит и интеграционные тесты

Добавил 5 интеграционных тестов в `tests/integration_test.rs` (через
`rocket::local::blocking::Client` с подменённым на `MockAiService`):

| Тест | Что проверяет |
|---|---|
| `test_summarize_happy_path` | связный текст из 4 предложений: статус 200, summary начинается с «Rust» и кончается точкой, `key_terms` непуст и ≤ 5 |
| `test_summarize_respects_max_terms` | `max_terms = 2` → не более двух терминов в ответе |
| `test_summarize_empty_text` | пустой `text` → ответ содержит `EMPTY_TEXT` |
| `test_summarize_missing_text_field` | JSON без поля `text` → 422 Unprocessable Entity |
| `test_summarize_zero_max_terms` | `max_terms = 0` → `key_terms` пуст, `summary` не пуст |

Финальный прогон `cargo test`:

```
running 9 tests   …  9 passed   (lib)
running 10 tests  … 10 passed   (unittests src/main.rs)
running 12 tests  … 12 passed   (integration_test, +5 моих)
running 13 tests  …  3 passed; 10 ignored   (doc-tests)

Итого: 34 passed; 0 failed.
```

### 5.2. Ручная проверка через curl

Поднял сервер `cargo run` (mock-режим) и проверил несколько сценариев:

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

Что получилось узнать и сделать в этой работе:

- Разобрался с тем, как Rocket связывает HTTP-эндпоинт с функцией через
  атрибутные макросы (`#[post("/path", format = "json", data = "<…>")]`)
  и как Rocket вместе с `serde` автоматически парсит JSON в Rust-структуру.
- Понял, зачем в учебном проекте сделана абстракция через
  трейт `AiService` и `Box<dyn AiService>`: добавляя свою тему, я ни разу
  не залез ни в `GigaChatService`, ни в `MockAiService` с точки зрения
  публичного API — расширил функциональность через **формат данных**,
  а не через трейт.
- Применил паттерн «безопасный фолбэк»: если ответ AI не соответствует
  ожидаемому формату с маркерами, клиент всё равно получит непустой
  `summary`, а не «крэш» с 500-ой.
- Написал интеграционные тесты, которые работают **без поднятия реального
  сервера** — через `rocket::local::blocking::Client`. Это удобнее, чем
  поднимать процесс и стрелять в него `curl` из теста.
- Закрепил формат коммитов с issue-ID, типом и приоритетом из README
  (`[<id>] <type>(P#): <title>` + описание + список изменённых файлов).

Что ещё было бы интересно сделать (выходит за рамки ЛР1):

- Подключить настоящий GigaChat и проверить, выдержит ли модель просьбу
  выводить маркеры `<<<SUMMARY>>>`/`<<<TERMS>>>` — реалистично ожидать,
  что в части случаев нет, и парсер тогда честно отработает фолбэк.
- Сделать ключевые термины умнее — например, отфильтровать стоп-слова
  («это», «как», «для»), которые сейчас попадают в результат, если
  текст длинный.
- Добавить параметр «максимальная длина выжимки» по символам.

В целом проект устроен прозрачно — добавление новой темы заняло у меня
не очень много времени, потому что архитектура уже была готова к такому
расширению. Это и есть полезный урок: хорошее разделение модулей и
аккуратный трейт-объект **окупаются**, когда работа выходит за рамки
исходного скоупа.
