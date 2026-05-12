//! Модуль обработчиков HTTP-запросов (Handlers).
//!
//! Этот модуль содержит функции-обработчики для различных эндпоинтов API.
//! Каждый обработчик отвечает за обработку конкретного типа запроса.
//!
//! # Для студентов: Архитектура веб-приложения
//!
//! В веб-разработке используется многослойная архитектура:
//!
//! ```text
//! HTTP-запрос
//!      ↓
//! ┌─────────────┐
//! │  Handlers   │  ← Этот модуль! Принимает запросы, возвращает ответы
//! └─────────────┘
//!      ↓
//! ┌─────────────┐
//! │  Services   │  ← Бизнес-логика (работа с AI)
//! └─────────────┘
//!      ↓
//! ┌─────────────┐
//! │   Models    │  ← Структуры данных (Request/Response)
//! └─────────────┘
//! ```
//!
//! Handler НЕ должен содержать бизнес-логику - только:
//! - Извлечение данных из запроса
//! - Вызов сервисов
//! - Формирование ответа
//!
//! # Макросы маршрутизации Rocket
//!
//! Rocket использует атрибуты-макросы для связывания функций с HTTP-эндпоинтами:
//!
//! - `#[get("/path")]` - обработчик GET-запросов
//! - `#[post("/path")]` - обработчик POST-запросов
//! - `#[catch(код)]` - обработчик ошибок (404, 500 и т.д.)

// ============================================================================
// ИМПОРТЫ
// ============================================================================

// Json - обёртка Rocket для автоматической сериализации/десериализации JSON.
// При возврате Json<T> Rocket автоматически:
// 1. Сериализует T в JSON
// 2. Устанавливает Content-Type: application/json
use rocket::serde::json::Json;

// State - механизм Dependency Injection в Rocket.
// Позволяет получить доступ к данным, переданным через .manage()
use rocket::State;

// Макросы маршрутизации - ОБЯЗАТЕЛЬНО импортировать явно!
// Rocket 0.5 требует явного импорта, в отличие от старых версий.
use rocket::{catch, get, options, post};

// tracing - библиотека структурированного логирования.
// info! - информационные сообщения
// error! - сообщения об ошибках
use tracing::{error, info};
use std::path::PathBuf;

use crate::config::AppConfig;
use crate::models::{
    AskRequest, AskResponse, ErrorResponse, HealthResponse, SummarizeRequest, SummarizeResponse,
};
use crate::services::{AiService, MOCK_SUMMARIZE_SUMMARY_TAG, MOCK_SUMMARIZE_TERMS_TAG};

/// Значение `max_terms` по умолчанию для `/summarize`,
/// если клиент не указал собственное.
const DEFAULT_MAX_KEY_TERMS: usize = 5;

/// Разбирает структурированный ответ AI на блоки `<<<SUMMARY>>>` и `<<<TERMS>>>`.
///
/// Если ответ не содержит ожидаемых маркеров (например, GigaChat ответил
/// «по-своему»), вся строка возвращается как `summary`, а `key_terms` —
/// пустым списком. Это безопасный фолбэк, при котором клиент всё равно
/// получает осмысленный результат.
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

// ============================================================================
// ОБРАБОТЧИКИ ЭНДПОИНТОВ
// ============================================================================

/// Обработчик корневого эндпоинта - возвращает информацию об API.
///
/// # Для студентов: Dependency Injection через State
///
/// Обратите внимание на параметр `config: &State<AppConfig>`.
///
/// ## Как это работает?
///
/// 1. В `main.rs` мы вызываем `.manage(config)` - передаём конфигурацию в Rocket
/// 2. Rocket сохраняет её во внутреннем хранилище
/// 3. В любом обработчике мы можем "запросить" эти данные через `State<T>`
/// 4. Rocket автоматически передаст нужный объект
///
/// ## Почему `&State<T>`, а не просто `T`?
///
/// - `State<T>` - это smart pointer (умный указатель)
/// - `&State<T>` - ссылка на этот указатель
/// - Данные НЕ копируются, мы получаем доступ к общему экземпляру
///
/// Это паттерн "Dependency Injection" - зависимости "внедряются" извне,
/// а не создаются внутри функции.
///
/// # Эндпоинт
///
/// `GET /`
///
/// # Примеры
///
/// ```bash
/// curl http://localhost:8000/
/// ```
#[get("/")]
pub fn index(config: &State<AppConfig>) -> String {
    format!(
        "🚀 {} v{}\n\n\
        {}\n\n\
        Доступные эндпоинты:\n\
        - GET  /           - Это сообщение\n\
        - GET  /health     - Проверка состояния сервера\n\
        - POST /ask        - Задать вопрос AI помощнику\n\
        - POST /summarize  - Краткий конспект текста + ключевые термины\n\n\
        Пример запроса:\n\
        curl -X POST http://localhost:8000/ask \\\n\
          -H \"Content-Type: application/json\" \\\n\
          -d '{{\"question\": \"Что такое Rust?\"}}'",
        config.application.name,
        config.application.version,
        config.application.description
    )
}

/// Обработчик эндпоинта проверки здоровья.
///
/// Возвращает информацию о состоянии сервера и его конфигурации.
/// Этот эндпоинт полезен для мониторинга и проверки доступности сервиса.
///
/// # Эндпоинт
///
/// `GET /health`
///
/// # Ответ
///
/// ```json
/// {
///   "status": "ok",
///   "version": "0.1.0",
///   "gigachat_enabled": true
/// }
/// ```
///
/// # Примеры
///
/// ```bash
/// curl http://localhost:8000/health
/// ```
#[get("/health")]
pub fn health(config: &State<AppConfig>) -> Json<HealthResponse> {
    info!("Health check requested");

    Json(HealthResponse {
        status: "ok".to_string(),
        version: config.application.version.clone(),
        gigachat_enabled: config.is_gigachat_enabled(),
    })
}

/// Обработчик эндпоинта для вопросов к AI - главная функциональность API.
///
/// # Для студентов: Разбор сложной сигнатуры
///
/// ```text
/// #[post("/ask", format = "json", data = "<request>")]
/// ^^^^^^ ^^^^^^  ^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^
///   |      |          |                |
///   |      |          |                +-- Имя параметра с телом запроса
///   |      |          +------------------- Принимаем только JSON
///   |      +------------------------------ URL-путь
///   +-------------------------------------- HTTP-метод POST
///
/// pub async fn ask(
///     request: Json<AskRequest>,            // Тело запроса (автоматически парсится из JSON)
///     ai_service: &State<Box<dyn AiService>>, // AI-сервис из State (DI)
/// ) -> Result<Json<AskResponse>, Json<ErrorResponse>>
///      ^^^^^^ ^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^
///        |          |                    |
///        |          |                    +-- Ошибка (если Result::Err)
///        |          +------------------------ Успех (если Result::Ok)
///        +----------------------------------- Тип Result позволяет вернуть или Ok, или Err
/// ```
///
/// ## Зачем `Box<dyn AiService>`?
///
/// В `State<>` хранится trait object - конкретный тип (GigaChat или Mock)
/// определяется во время выполнения программы. См. модуль `services`.
///
/// ## Автоматическая обработка JSON
///
/// Rocket + Serde делают магию:
/// 1. Клиент отправляет: `{"question": "Что такое Rust?"}`
/// 2. Rocket видит `Json<AskRequest>` и автоматически парсит JSON в структуру
/// 3. Мы работаем с `request.question` как с обычным `String`
/// 4. При возврате `Json<AskResponse>` - обратная сериализация в JSON
///
/// # Эндпоинт
///
/// `POST /ask`
///
/// # Примеры
///
/// ```bash
/// curl -X POST http://localhost:8000/ask \
///   -H "Content-Type: application/json" \
///   -d '{"question": "Что такое Rust?"}'
/// ```
#[post("/ask", format = "json", data = "<request>")]
pub async fn ask(
    request: Json<AskRequest>,
    ai_service: &State<Box<dyn AiService>>,
) -> Result<Json<AskResponse>, Json<ErrorResponse>> {
    let question = &request.question;

    // Логируем входящий запрос
    info!("Received question: {}", question);

    // Check that question is not empty
    if question.trim().is_empty() {
        error!("Empty question received");
        return Err(Json(ErrorResponse::with_code(
            "Question cannot be empty",
            "EMPTY_QUESTION",
        )));
    }

    // Отправляем вопрос в AI сервис и ждём ответ
    match ai_service.ask(question).await {
        Ok(answer) => {
            info!("Successfully got answer from {}", ai_service.name());
            
            // ═══════════════════════════════════════════════════════════════
            // Для студентов: ЗДЕСЬ создаётся AskResponse!
            // ═══════════════════════════════════════════════════════════════
            //
            // AskResponse НЕ десериализуется из внешнего API.
            // Мы создаём его ПРОГРАММНО прямо здесь:
            //
            // - `answer` - String, полученный от ai_service.ask()
            //   (либо от реального GigaChat, либо от MockAiService)
            //
            // - `source` - НАШЕ внутреннее поле, GigaChat API о нём не знает!
            //   ai_service.name() возвращает "GigaChat" или "Mock AI Service"
            //   .to_lowercase() превращает в "gigachat" или "mock ai service"
            //
            // Затем Json(AskResponse{...}) автоматически сериализуется в JSON
            // и отправляется клиенту.
            // ═══════════════════════════════════════════════════════════════
            
            Ok(Json(AskResponse {
                answer,                                  // ← из AI сервиса
                source: ai_service.name().to_lowercase(), // ← наше поле
                system_prompt_applied: ai_service.system_prompt_applied(),
            }))
        }
        Err(e) => {
            error!("Error getting answer: {}", e);
            Err(Json(ErrorResponse::with_code(
                format!("Failed to get answer: {}", e),
                "AI_SERVICE_ERROR",
            )))
        }
    }
}

/// Обработчик эндпоинта конспектирования текста («Умный конспектор», ЛР1 тема 1).
///
/// Принимает произвольный текст в поле `text` и возвращает краткую выжимку
/// плюс список ключевых терминов. В качестве «движка» используется
/// `AiService` — тот же сервис, что и для `/ask` (Mock или GigaChat).
///
/// # Эндпоинт
///
/// `POST /summarize`
///
/// # Пример запроса
///
/// ```bash
/// curl -X POST http://localhost:8000/summarize \
///   -H "Content-Type: application/json" \
///   -d '{"text": "Rust — современный системный язык программирования …", "max_terms": 3}'
/// ```
///
/// # Текущее поведение (этап 3a лабораторной)
///
/// На этом этапе handler делегирует генерацию выжимки в `ai_service.ask()`
/// с заранее подготовленным промптом. Поле `key_terms` пока заполняется
/// пустым списком — структурированный разбор ответа добавляется на следующем
/// этапе ЛР1 («mock-логика конспектирования»).
#[post("/summarize", format = "json", data = "<request>")]
pub async fn summarize(
    request: Json<SummarizeRequest>,
    ai_service: &State<Box<dyn AiService>>,
) -> Result<Json<SummarizeResponse>, Json<ErrorResponse>> {
    let text = request.text.trim();

    info!("Received summarize request ({} chars)", text.len());

    if text.is_empty() {
        error!("Empty text for summarize");
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
            info!("Summarize: got answer from {}", ai_service.name());
            let (summary, key_terms) = parse_summary_payload(&answer, max_terms);
            Ok(Json(SummarizeResponse {
                summary,
                key_terms,
                source: ai_service.name().to_lowercase(),
            }))
        }
        Err(e) => {
            error!("Summarize: AI service error: {}", e);
            Err(Json(ErrorResponse::with_code(
                format!("Failed to summarize: {e}"),
                "AI_SERVICE_ERROR",
            )))
        }
    }
}

/// Обработчик preflight-запросов для CORS (OPTIONS).
///
/// Rocket не создаёт OPTIONS‑маршруты автоматически, поэтому браузерный
/// preflight завершался 404. Этот handler возвращает 204 No Content
/// для любых путей API, позволяя браузеру продолжить POST/GET запрос.
#[options("/<_path..>")]
pub fn cors_preflight(_path: PathBuf) -> rocket::http::Status {
    rocket::http::Status::NoContent
}

// ============================================================================
// ОБРАБОТЧИКИ ОШИБОК (CATCHERS)
// ============================================================================

// Для студентов: Error Catchers в Rocket
//
// "Catchers" (ловцы) - это обработчики ошибок HTTP.
// Они вызываются автоматически, когда:
// - Эндпоинт не найден (404)
// - Произошла ошибка сервера (500)
// - Неверный формат запроса (400, 422)
//
// Без catchers Rocket вернёт HTML-страницу с ошибкой.
// С catchers мы возвращаем JSON - это важно для API!

/// Обработчик для несуществующих эндпоинтов (404 Not Found).
///
/// Вызывается, когда клиент запрашивает URL, для которого нет обработчика.
///
/// # Примеры
///
/// ```bash
/// curl http://localhost:8000/nonexistent
/// # Вернёт: {"error": "Endpoint not found...", "code": "NOT_FOUND"}
/// ```
#[catch(404)]
pub fn not_found() -> Json<ErrorResponse> {
    Json(ErrorResponse::with_code(
        "Endpoint not found. Use GET / to see available endpoints.",
        "NOT_FOUND",
    ))
}

/// Обработчик для внутренних ошибок сервера (500 Internal Server Error).
///
/// Вызывается при необработанных исключениях (паниках) в коде.
/// В продакшене важно логировать такие ошибки для отладки.
#[catch(500)]
pub fn internal_error() -> Json<ErrorResponse> {
    Json(ErrorResponse::with_code(
        "Internal server error",
        "INTERNAL_ERROR",
    ))
}

/// Обработчик для ошибок валидации запроса (422 Unprocessable Entity).
///
/// # Для студентов: Когда возникает 422?
///
/// Эта ошибка возникает, когда:
/// - JSON синтаксически корректен, но не соответствует ожидаемой структуре
/// - Отсутствуют обязательные поля
/// - Типы полей не совпадают (например, строка вместо числа)
///
/// Примеры запросов, вызывающих 422:
/// ```json
/// {}                           // Отсутствует поле "question"
/// {"question": 123}            // question должен быть строкой
/// {"questions": "..."}         // Опечатка в имени поля
/// ```
#[catch(422)]
pub fn unprocessable_entity() -> Json<ErrorResponse> {
    Json(ErrorResponse::with_code(
        "Invalid request format. Check your JSON.",
        "INVALID_REQUEST",
    ))
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

/// # Для студентов: Тестирование Rocket-приложений
///
/// Rocket предоставляет специальный "тестовый клиент" (`local::blocking::Client`),
/// который позволяет отправлять HTTP-запросы БЕЗ реального сервера.
///
/// ```text
/// Обычное тестирование:
///   1. Запустить сервер (cargo run)
///   2. Отправить curl-запрос
///   3. Проверить ответ
///
/// Тестирование с Rocket Client:
///   1. Создать "виртуальный" Rocket
///   2. Client отправляет запросы напрямую в handlers
///   3. Быстро, изолированно, без сети
/// ```
#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::handlers::{health, index};
    // routes! - макрос, который создаёт Vec маршрутов из функций-handlers
    use rocket::{routes, local::blocking::Client, Build, Rocket};

    /// Создаёт тестовый экземпляр Rocket (без запуска сервера).
    ///
    /// # Для студентов: Почему `#[test]`, а не `#[tokio::test]`?
    ///
    /// Мы используем `local::blocking::Client` - СИНХРОННУЮ версию клиента.
    /// Она проще для тестов и не требует async runtime.
    ///
    /// Rocket также предоставляет `local::asynchronous::Client` для async-тестов,
    /// но для простых тестов blocking-версия удобнее.
    fn create_test_rocket() -> Rocket<Build> {
        let config = AppConfig::load().unwrap_or_else(|_| {
            panic!("Не удалось загрузить конфигурацию для тестов");
        });

        // rocket::build() создаёт Rocket в состоянии Build (ещё не запущен)
        // .manage() добавляет State
        // .mount() регистрирует маршруты
        rocket::build()
            .manage(config)
            .mount("/", routes![index, health])  // routes! - макрос!
    }

    /// Тест: GET / возвращает 200 OK
    #[test]
    fn test_index() {
        // Client::tracked создаёт клиент с отслеживанием cookies/sessions
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        // .get("/") - создаёт GET-запрос
        // .dispatch() - "отправляет" его (на самом деле вызывает handler напрямую)
        let response = client.get("/").dispatch();
        assert_eq!(response.status().code, 200);
    }

    /// Тест: GET /health возвращает 200 OK
    #[test]
    fn test_health() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        let response = client.get("/health").dispatch();
        assert_eq!(response.status().code, 200);
    }
}
