//! Интеграционные тесты для демонстрационного приложения.
//!
//! # Для студентов: Интеграционные vs Unit тесты
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │               ВИДЫ ТЕСТОВ В RUST-ПРОЕКТЕ                           │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                     │
//! │  UNIT ТЕСТЫ (модульные)          INTEGRATION ТЕСТЫ (интеграционные)│
//! │  Расположение: src/**/*.rs        Расположение: tests/*.rs         │
//! │  Внутри #[cfg(test)] mod tests    Отдельные файлы                  │
//! │                                                                     │
//! │  ┌─────────────┐                  ┌─────────────────────────────┐  │
//! │  │ Тестируют   │                  │ Тестируют взаимодействие   │  │
//! │  │ одну        │                  │ нескольких компонентов     │  │
//! │  │ функцию/    │                  │ как единое целое           │  │
//! │  │ модуль      │                  │                             │  │
//! │  └─────────────┘                  └─────────────────────────────┘  │
//! │                                                                     │
//! │  Пример:                          Пример:                          │
//! │  - test_mock_service()            - test_ask_endpoint_valid()      │
//! │  - test_error_creation()          - test_full_request_flow()       │
//! │                                                                     │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Почему интеграционные тесты в отдельной папке?
//!
//! 1. **Видят проект как внешний пользователь** - используют `use rust_gigachat_demo::...`
//! 2. **Не имеют доступа к приватным элементам** - только pub API
//! 3. **Компилируются отдельно** - каждый файл = отдельный crate
//!
//! ## Макросы routes! и catchers!
//!
//! ```rust,ignore
//! routes![index, health, ask]  // Создаёт Vec<Route> из функций
//! catchers![not_found, ...]    // Создаёт Vec<Catcher> из функций
//! ```
//!
//! Эти макросы преобразуют функции с атрибутами #[get], #[post], #[catch]
//! в объекты, которые Rocket может использовать для маршрутизации.

use rocket::{routes, catchers};
use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;

// Импортируем из НАШЕГО крейта (как внешние пользователи)
use rust_gigachat_demo::config::AppConfig;
use rust_gigachat_demo::handlers::{
    ask, health, index, internal_error, not_found, summarize, unprocessable_entity,
};
use rust_gigachat_demo::services::MockAiService;

/// Создаёт тестовый экземпляр Rocket с mock-сервисом.
///
/// # Для студентов: Тестовая изоляция
///
/// Мы ВСЕГДА используем MockAiService в тестах, даже если gigachat.enabled=true.
/// Причины:
/// 1. **Изоляция** - тесты не зависят от внешних сервисов
/// 2. **Скорость** - нет сетевых задержек
/// 3. **Детерминизм** - одинаковый результат при каждом запуске
/// 4. **Бесплатно** - не тратим токены GigaChat API
fn create_test_client() -> Client {
    let config = AppConfig::load().expect("Failed to load config");
    
    // ВСЕГДА mock для тестов - это best practice!
    let ai_service: Box<dyn rust_gigachat_demo::services::AiService> = Box::new(MockAiService::new());

    let rocket = rocket::build()
        .manage(config)                    // State<AppConfig>
        .manage(ai_service)                // State<Box<dyn AiService>>
        .mount("/", routes![index, health, ask, summarize])  // routes! - макрос!
        .register("/", catchers![not_found, internal_error, unprocessable_entity]);

    // Client::tracked отслеживает cookies между запросами
    Client::tracked(rocket).expect("valid rocket instance")
}

// ============================================================================
// ТЕСТЫ ЭНДПОИНТОВ
// ============================================================================

/// Тест: GET / возвращает информацию об API
#[test]
fn test_index_endpoint() {
    let client = create_test_client();
    let response = client.get("/").dispatch();

    assert_eq!(response.status(), Status::Ok);
    
    let body = response.into_string().unwrap();
    assert!(body.contains("Доступные эндпоинты"));
}

#[test]
fn test_health_endpoint() {
    let client = create_test_client();
    let response = client.get("/health").dispatch();

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    
    let body = response.into_string().unwrap();
    assert!(body.contains("status"));
    assert!(body.contains("version"));
}

#[test]
fn test_ask_endpoint_valid_question() {
    let client = create_test_client();
    let response = client
        .post("/ask")
        .header(ContentType::JSON)
        .body(r#"{"question": "Что такое Rust?"}"#)
        .dispatch();

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    
    let body = response.into_string().unwrap();
    assert!(body.contains("answer"));
    assert!(body.contains("source"));
}

#[test]
fn test_ask_endpoint_empty_question() {
    let client = create_test_client();
    let response = client
        .post("/ask")
        .header(ContentType::JSON)
        .body(r#"{"question": ""}"#)
        .dispatch();

    // Ожидаем ошибку для пустого вопроса
    let body = response.into_string().unwrap();
    assert!(body.contains("error"));
}

#[test]
fn test_ask_endpoint_invalid_json() {
    let client = create_test_client();
    let response = client
        .post("/ask")
        .header(ContentType::JSON)
        .body(r#"{"invalid": json}"#)
        .dispatch();

    // Rocket вернёт 400 для невалидного JSON
    assert_eq!(response.status(), Status::BadRequest);
}

#[test]
fn test_not_found_endpoint() {
    let client = create_test_client();
    let response = client.get("/nonexistent").dispatch();

    assert_eq!(response.status(), Status::NotFound);
    
    let body = response.into_string().unwrap();
    assert!(body.contains("error"));
}

// ============================================================================
// ТЕСТЫ ЭНДПОИНТА /summarize («Умный конспектор», ЛР1 тема 1)
// ============================================================================

/// Хелпер: разбирает JSON-ответ /summarize в три поля для удобства проверок.
fn parse_summarize_body(body: &str) -> (String, Vec<String>, String) {
    let v: serde_json::Value = serde_json::from_str(body).expect("response must be valid JSON");
    let summary = v["summary"].as_str().unwrap_or_default().to_string();
    let key_terms: Vec<String> = v["key_terms"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let source = v["source"].as_str().unwrap_or_default().to_string();
    (summary, key_terms, source)
}

/// Happy path: связный текст из нескольких предложений →
/// summary = первое предложение, key_terms = до 5 непустых слов.
#[test]
fn test_summarize_happy_path() {
    let client = create_test_client();
    let body = r#"{
        "text": "Rust — современный системный язык программирования. Он обеспечивает безопасность памяти без сборщика мусора. Rust популярен для веб-серверов, встраиваемых систем и инструментов командной строки. Сообщество Rust большое и активное."
    }"#;

    let response = client
        .post("/summarize")
        .header(ContentType::JSON)
        .body(body)
        .dispatch();

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));

    let (summary, key_terms, source) = parse_summarize_body(&response.into_string().unwrap());
    assert!(
        summary.starts_with("Rust"),
        "summary должен начинаться с первого предложения, был: {summary}"
    );
    assert!(
        summary.ends_with('.'),
        "summary должен заканчиваться точкой, был: {summary}"
    );
    assert!(
        !key_terms.is_empty() && key_terms.len() <= 5,
        "key_terms должны быть непусты и ≤ 5, было: {key_terms:?}"
    );
    assert_eq!(source, "mock ai service");
}

/// Параметр `max_terms` уважается — handler ограничивает выдачу.
#[test]
fn test_summarize_respects_max_terms() {
    let client = create_test_client();
    let body = r#"{
        "text": "Rocket — это веб-фреймворк для языка Rust. Rocket делает разработку быстрой и безопасной. Rocket поддерживает асинхронность, JSON и кастомные fairings.",
        "max_terms": 2
    }"#;

    let response = client
        .post("/summarize")
        .header(ContentType::JSON)
        .body(body)
        .dispatch();

    assert_eq!(response.status(), Status::Ok);
    let (_, key_terms, _) = parse_summarize_body(&response.into_string().unwrap());
    assert!(
        key_terms.len() <= 2,
        "key_terms должны быть не более 2, было: {key_terms:?}"
    );
}

/// Пустой `text` → структурированная ошибка `EMPTY_TEXT`.
#[test]
fn test_summarize_empty_text() {
    let client = create_test_client();
    let response = client
        .post("/summarize")
        .header(ContentType::JSON)
        .body(r#"{"text": ""}"#)
        .dispatch();

    let body = response.into_string().unwrap();
    assert!(body.contains("\"error\""));
    assert!(body.contains("\"EMPTY_TEXT\""));
}

/// JSON без поля `text` → 422 от catcher’а validation.
#[test]
fn test_summarize_missing_text_field() {
    let client = create_test_client();
    let response = client
        .post("/summarize")
        .header(ContentType::JSON)
        .body(r#"{"max_terms": 3}"#)
        .dispatch();

    assert_eq!(response.status(), Status::UnprocessableEntity);
    let body = response.into_string().unwrap();
    assert!(body.contains("\"error\""));
}

/// `max_terms = 0` → пустой список ключевых терминов, summary всё ещё есть.
#[test]
fn test_summarize_zero_max_terms() {
    let client = create_test_client();
    let body = r#"{"text": "Rust — это язык программирования.", "max_terms": 0}"#;

    let response = client
        .post("/summarize")
        .header(ContentType::JSON)
        .body(body)
        .dispatch();

    assert_eq!(response.status(), Status::Ok);
    let (summary, key_terms, _) = parse_summarize_body(&response.into_string().unwrap());
    assert!(!summary.is_empty(), "summary должен быть непуст");
    assert!(
        key_terms.is_empty(),
        "при max_terms=0 список должен быть пуст, было: {key_terms:?}"
    );
}

#[test]
fn test_mock_service_responses() {
    let client = create_test_client();
    
    // Тест на вопрос про Rust
    let response = client
        .post("/ask")
        .header(ContentType::JSON)
        .body(r#"{"question": "Что такое Rust?"}"#)
        .dispatch();
    
    let body = response.into_string().unwrap();
    assert!(body.contains("Rust"));
    
    // Тест на вопрос про Rocket
    let response = client
        .post("/ask")
        .header(ContentType::JSON)
        .body(r#"{"question": "Что такое Rocket?"}"#)
        .dispatch();
    
    let body = response.into_string().unwrap();
    assert!(body.contains("Rocket"));
}
