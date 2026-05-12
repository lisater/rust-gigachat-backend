//! Модуль моделей данных (Data Transfer Objects - DTO).
//!
//! Этот модуль содержит структуры данных, используемые для обмена информацией
//! между клиентом и сервером через API.
//!
//! # Для студентов: Что такое DTO?
//!
//! DTO (Data Transfer Object) - это объекты, предназначенные ТОЛЬКО для передачи данных.
//! Они не содержат бизнес-логики, только поля и методы сериализации.
//!
//! # Serde - сериализация в Rust
//!
//! Serde (SERialize/DEserialize) - самая популярная библиотека для работы с данными.
//!
//! ## Ключевой принцип: используйте ТОЛЬКО нужные трейты!
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    НАПРАВЛЕНИЕ ДАННЫХ                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   КЛИЕНТ                              СЕРВЕР                    │
//! │                                                                 │
//! │   ┌─────────┐    HTTP Request    ┌─────────────┐               │
//! │   │ Browser │ ──── JSON ──────>  │ AskRequest  │               │
//! │   │   или   │                    │ Deserialize │ ← только чтение│
//! │   │  curl   │                    └─────────────┘               │
//! │   │         │                                                   │
//! │   │         │    HTTP Response   ┌─────────────┐               │
//! │   │         │ <──── JSON ──────  │ AskResponse │               │
//! │   └─────────┘                    │ Serialize   │ ← только запись│
//! │                                  └─────────────┘               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## ВАЖНО: Не добавляйте лишние derive!
//!
//! ```rust,ignore
//! // ❌ ПЛОХО: избыточные трейты
//! #[derive(Debug, Deserialize, Serialize, Clone)]  // Serialize не нужен для Request!
//! pub struct AskRequest { ... }
//!
//! // ✅ ХОРОШО: только необходимое
//! #[derive(Debug, Deserialize)]  // Request только читаем из JSON
//! pub struct AskRequest { ... }
//!
//! #[derive(Debug, Serialize)]    // Response только пишем в JSON
//! pub struct AskResponse { ... }
//! ```
//!
//! Почему это важно:
//! 1. **Читаемость** - сразу видно назначение структуры
//! 2. **Безопасность** - нельзя случайно сериализовать Request
//! 3. **Компиляция** - меньше кода = быстрее компиляция
//!
//! # Атрибут `#[serde(crate = "rocket::serde")]`
//!
//! Rocket 0.5 реэкспортирует свою версию serde. Этот атрибут указывает:
//! "Используй serde из Rocket, а не из корня проекта".
//!
//! **Когда нужен:**
//! - Структуры, используемые с `Json<T>` в Rocket handlers
//!
//! **Когда НЕ нужен:**
//! - Структуры конфигурации (config crate имеет свой serde)
//! - Структуры для других библиотек

// ============================================================================
// ИМПОРТЫ
// ============================================================================

// Serialize - трейт для преобразования структуры → JSON (сериализация)
// Deserialize - трейт для создания структуры ← JSON (десериализация)
use serde::{Deserialize, Serialize};

// ============================================================================
// МОДЕЛИ ЗАПРОСОВ (REQUEST) - только Deserialize!
// ============================================================================

/// Запрос к API для получения ответа на вопрос.
///
/// # Для студентов: Почему именно такой набор derive?
///
/// ```text
/// #[derive(Debug, Deserialize)]
///          ^^^^^  ^^^^^^^^^^^
///            |         |
///            |         +-- Deserialize: создаём структуру ИЗ JSON
///            |             (клиент отправляет нам JSON → мы читаем)
///            |
///            +------------ Debug: для отладочного вывода ({:?})
///                          Полезно для логирования: info!("{:?}", request)
/// ```
///
/// ## Чего здесь НЕТ и почему:
///
/// - **Serialize** - НЕ НУЖЕН! Мы не отправляем Request обратно клиенту
/// - **Clone** - НЕ НУЖЕН! Мы не копируем запрос, просто читаем из него
///
/// ## Атрибут `#[serde(crate = "rocket::serde")]`
///
/// Rocket 0.5 поставляет свою версию serde. Без этого атрибута возникнет
/// конфликт: Rocket ожидает "свой" Deserialize, а мы используем "чужой".
///
/// ```text
/// Cargo.toml:  serde = "1.0"        ← версия A
/// Rocket:      rocket::serde        ← версия B (реэкспорт)
///
/// Без атрибута: derive использует версию A
/// С атрибутом:  derive использует версию B ← правильно для Rocket!
/// ```
///
/// # Пример: как Rocket обрабатывает запрос
///
/// ```text
/// POST /ask
/// Content-Type: application/json
/// {"question": "Что такое Rust?"}
///
///      ↓ Rocket видит Json<AskRequest>
///      ↓ Вызывает Deserialize::deserialize()
///      ↓ Создаёт AskRequest { question: "Что такое Rust?" }
/// ```
#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AskRequest {
    /// Вопрос пользователя.
    /// Serde автоматически сопоставляет JSON-поле "question" с этим полем.
    pub question: String,
}

// ============================================================================
// МОДЕЛИ ОТВЕТОВ (RESPONSE) - только Serialize!
// ============================================================================

/// Успешный ответ API на вопрос пользователя.
///
/// # Для студентов: Откуда берутся значения полей?
///
/// **ВАЖНО:** Эта структура НЕ десериализуется из внешнего API!
/// Она создаётся ПРОГРАММНО в нашем коде (в `handlers/mod.rs`).
///
/// ## Полный поток данных
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────────────┐
/// │                         ПОТОК ДАННЫХ                                    │
/// ├─────────────────────────────────────────────────────────────────────────┤
/// │                                                                         │
/// │  1. Клиент отправляет запрос:                                          │
/// │     POST /ask  {"question": "Что такое Rust?"}                         │
/// │                        │                                                │
/// │                        ▼                                                │
/// │  2. Rocket десериализует в AskRequest                                  │
/// │     AskRequest { question: "Что такое Rust?" }                         │
/// │                        │                                                │
/// │                        ▼                                                │
/// │  3. Handler вызывает ai_service.ask(question)                          │
/// │     ┌──────────────────┴──────────────────┐                            │
/// │     │                                     │                            │
/// │     ▼                                     ▼                            │
/// │  GigaChatService                    MockAiService                      │
/// │  (реальный API)                     (заглушка)                         │
/// │     │                                     │                            │
/// │     │ gigalib отправляет                 │ возвращает                  │
/// │     │ запрос к GigaChat API              │ захардкоженный текст        │
/// │     │                                     │                            │
/// │     ▼                                     ▼                            │
/// │  Возвращает String              Возвращает String                      │
/// │  "Rust - это язык..."           "Rust is a systems..."                 │
/// │     │                                     │                            │
/// │     └──────────────────┬──────────────────┘                            │
/// │                        │                                                │
/// │                        ▼                                                │
/// │  4. Handler СОЗДАЁТ AskResponse (см. handlers/mod.rs:226-229):         │
/// │     ┌─────────────────────────────────────────────────────────┐        │
/// │     │ Ok(Json(AskResponse {                                   │        │
/// │     │     answer,                    // ← String из сервиса   │        │
/// │     │     source: ai_service.name()  // ← "GigaChat" или      │        │
/// │     │              .to_lowercase(),  //   "Mock AI Service"   │        │
/// │     │ }))                                                     │        │
/// │     └─────────────────────────────────────────────────────────┘        │
/// │                        │                                                │
/// │                        ▼                                                │
/// │  5. Rocket сериализует AskResponse в JSON:                             │
/// │     {"answer": "Rust - это язык...", "source": "gigachat"}             │
/// │                        │                                                │
/// │                        ▼                                                │
/// │  6. JSON отправляется клиенту                                          │
/// │                                                                         │
/// └─────────────────────────────────────────────────────────────────────────┘
/// ```
///
/// ## Откуда берётся `answer`?
///
/// - **GigaChatService**: из ответа реального GigaChat API (через gigalib)
/// - **MockAiService**: из захардкоженных строк в `services/mod.rs`
///
/// ## Откуда берётся `source`?
///
/// Метод `ai_service.name()` возвращает:
/// - `"GigaChat"` для GigaChatService
/// - `"Mock AI Service"` для MockAiService
///
/// Это НАШЕ внутреннее поле для отладки, GigaChat API о нём не знает!
///
/// ## Почему только Serialize?
///
/// Мы СОЗДАЁМ эту структуру в коде и ОТПРАВЛЯЕМ клиенту.
/// Мы никогда не ПОЛУЧАЕМ её извне. Поэтому Deserialize не нужен.
///
/// # Пример JSON-ответа
///
/// ```json
/// {
///   "answer": "Rust - это системный язык...",
///   "source": "gigachat"
/// }
/// ```
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct AskResponse {
    /// Текст ответа от AI.
    /// 
    /// Источник значения:
    /// - GigaChat: из HTTP-ответа GigaChat API (поле `content` в JSON)
    /// - Mock: из захардкоженных строк в MockAiService::ask()
    pub answer: String,
    
    /// Источник ответа - ВНУТРЕННЕЕ поле нашего приложения.
    /// 
    /// НЕ приходит от GigaChat API! Заполняется в handler:
    /// `source: ai_service.name().to_lowercase()`
    /// 
    /// Значения: "gigachat" или "mock ai service"
    pub source: String,

    /// Признак того, что системный промпт был применён к запросу.
    ///
    /// Сам текст системного промпта НЕ возвращается клиенту.
    pub system_prompt_applied: bool,
}

/// Информация о состоянии сервера (health check).
///
/// # Для студентов: Health Check эндпоинт
///
/// Health check - стандартный паттерн в микросервисах.
/// Используется для:
/// - Мониторинга (Prometheus, Grafana)
/// - Load balancer'ов (проверка "жив ли сервис?")
/// - Kubernetes liveness/readiness probes
///
/// ## Derive-атрибуты
///
/// ```text
/// #[derive(Debug, Serialize)]
///                 ^^^^^^^^^
///                 Только Serialize - это исходящий ответ
/// ```
///
/// # Пример JSON
///
/// ```json
/// {
///   "status": "ok",
///   "version": "0.1.0",
///   "gigachat_enabled": true
/// }
/// ```
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct HealthResponse {
    /// Статус сервера: "ok" или "error"
    pub status: String,
    
    /// Версия приложения (из config.toml)
    pub version: String,
    
    /// Флаг: используется реальный GigaChat или mock
    pub gigachat_enabled: bool,
}

/// Ответ с ошибкой - стандартный формат для всех ошибок API.
///
/// # Для студентов: Единый формат ошибок
///
/// В хорошем API ВСЕ ошибки возвращаются в одинаковом формате.
/// Клиент всегда знает, какую структуру ожидать при ошибке.
///
/// ## Derive-атрибуты
///
/// ```text
/// #[derive(Debug, Serialize)]
///                 ^^^^^^^^^
///                 Только Serialize - ошибки мы только отправляем
/// ```
///
/// ## Полевой атрибут `#[serde(skip_serializing_if = "...")]`
///
/// Этот атрибут применяется к ПОЛЮ, не к структуре.
/// Он управляет условной сериализацией:
///
/// ```text
/// #[serde(skip_serializing_if = "Option::is_none")]
///         ^^^^^^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^
///         "пропустить при          условие: функция,
///          сериализации, если"     возвращающая bool
/// ```
///
/// ### Как это работает:
///
/// ```rust,ignore
/// // При сериализации serde вызывает: Option::is_none(&self.code)
/// // Если true  → поле НЕ включается в JSON
/// // Если false → поле включается в JSON
/// ```
///
/// ### Результат:
///
/// ```text
/// ErrorResponse { error: "...", code: None }
///     → {"error": "..."}                      // code пропущен!
///
/// ErrorResponse { error: "...", code: Some("NOT_FOUND") }
///     → {"error": "...", "code": "NOT_FOUND"} // code включён
/// ```
///
/// ### Зачем это нужно?
///
/// 1. **Чистый JSON** - нет `"code": null`
/// 2. **Экономия трафика** - меньше байт
/// 3. **Семантика** - отсутствие поля ≠ поле со значением null
///
/// ## Другие полезные атрибуты serde для полей:
///
/// ```rust,ignore
/// #[serde(rename = "errorMessage")]  // JSON-имя отличается от Rust-имени
/// #[serde(default)]                   // Использовать Default::default() если поля нет
/// #[serde(skip)]                      // Всегда пропускать это поле
/// #[serde(flatten)]                   // "Развернуть" вложенную структуру
/// ```
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct ErrorResponse {
    /// Человекочитаемое описание ошибки (для пользователя/логов)
    pub error: String,
    
    /// Машиночитаемый код ошибки (для программной обработки).
    /// Пропускается в JSON, если равен None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// ============================================================================
// МОДЕЛИ ДЛЯ /summarize («Умный конспектор» — ЛР1 тема 1)
// ============================================================================

/// Запрос на конспектирование текста.
///
/// Принимается обработчиком `POST /summarize` (см. `handlers::summarize`).
/// Поле `text` обязательно, `max_terms` — опциональное ограничение на размер
/// списка ключевых терминов в ответе.
///
/// # Для студентов: derive только Deserialize
///
/// `SummarizeRequest` приходит к нам ИЗ JSON-тела HTTP-запроса, и мы никогда
/// не отправляем его обратно клиенту. Поэтому в `#[derive(...)]` нет
/// `Serialize`, а только `Deserialize`. Принцип «только нужные трейты» —
/// смотри раздел «Не добавляйте лишние derive!» в начале этого модуля.
///
/// # Атрибут `#[serde(default)]` на `max_terms`
///
/// Поле `max_terms` объявлено как `Option<usize>` и помечено
/// `#[serde(default)]`. Это значит:
///
/// - если клиент **передал** поле — мы получим `Some(N)`;
/// - если клиент **не передал** поле — Serde подставит `Default::default()`,
///   то есть `None`.
///
/// Без `#[serde(default)]` отсутствие поля в JSON привело бы к ошибке
/// десериализации.
///
/// # Пример JSON-запроса
///
/// ```json
/// {
///   "text": "Rust — современный системный язык программирования. Он …",
///   "max_terms": 5
/// }
/// ```
///
/// или без `max_terms`:
///
/// ```json
/// { "text": "Rust — современный системный язык программирования." }
/// ```
#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct SummarizeRequest {
    /// Исходный текст для конспектирования (произвольной длины).
    pub text: String,

    /// Желаемое максимальное число ключевых терминов в ответе.
    /// Если не передано — handler использует `DEFAULT_MAX_KEY_TERMS` (5).
    #[serde(default)]
    pub max_terms: Option<usize>,
}

/// Ответ конспектора: краткая выжимка и список ключевых терминов.
///
/// Создаётся программно в `handlers::summarize` после получения ответа от
/// `AiService`. Никогда не приходит «извне» — поэтому derive только
/// `Serialize`, как и у `AskResponse`.
///
/// # Поток данных
///
/// ```text
/// POST /summarize  {"text": "…", "max_terms": 3}
///        │
///        ▼
/// handlers::summarize → строит промпт «Сделай краткий конспект …»
///        │
///        ▼
/// ai_service.ask(prompt)
///        │
///        ▼
/// MockAiService возвращает «<<<SUMMARY>>>… <<<TERMS>>>…»
///   (для GigaChat ответ может быть произвольным — handler возьмёт его
///    как summary целиком, а key_terms останется пустым)
///        │
///        ▼
/// handlers::summarize парсит ответ и создаёт SummarizeResponse
///        │
///        ▼
/// Rocket сериализует → клиент получает JSON
/// ```
///
/// # Пример JSON-ответа
///
/// ```json
/// {
///   "summary": "Rust — современный системный язык программирования.",
///   "key_terms": ["безопасность", "веб-серверов", "встраиваемых"],
///   "source": "mock ai service"
/// }
/// ```
///
/// # Поле `source`
///
/// Семантика совпадает с `AskResponse::source` — показывает, откуда пришёл
/// результат: `"gigachat"` или `"mock ai service"`. Это **наше внутреннее**
/// поле, GigaChat API о нём не знает.
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct SummarizeResponse {
    /// Краткая выжимка текста (1–2 предложения).
    pub summary: String,

    /// Список ключевых терминов, длиной не больше запрошенного `max_terms`.
    /// Может быть пустым, если AI вернул нестандартный ответ без маркеров.
    pub key_terms: Vec<String>,

    /// Источник результата: `gigachat` или `mock ai service`.
    pub source: String,
}

impl ErrorResponse {
    /// Создаёт новый ответ с ошибкой (без кода).
    ///
    /// # Для студентов: Паттерн `impl Into<T>`
    ///
    /// Сигнатура `error: impl Into<String>` означает:
    /// "Принимаю любой тип, который можно преобразовать в String"
    ///
    /// Это позволяет вызывать метод с разными типами:
    ///
    /// ```rust,ignore
    /// ErrorResponse::new("строковый литерал");     // &str
    /// ErrorResponse::new(String::from("String")); // String
    /// ErrorResponse::new(format!("форматированная {}", var)); // String
    /// ```
    ///
    /// Без `impl Into<String>` пришлось бы всегда передавать `String`
    /// или создавать несколько версий метода.
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),  // .into() преобразует в String
            code: None,
        }
    }

    /// Создаёт новый ответ с ошибкой и кодом.
    ///
    /// # Пример использования
    ///
    /// ```rust,ignore
    /// ErrorResponse::with_code("Endpoint not found", "NOT_FOUND");
    /// ```
    pub fn with_code(error: impl Into<String>, code: impl Into<String>) -> Self {
        let mut response = Self::new(error);
        response.code = Some(code.into());
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тест ДЕСЕРИАЛИЗАЦИИ AskRequest (JSON → структура).
    ///
    /// # Для студентов: Правильное тестирование
    ///
    /// AskRequest имеет только Deserialize, поэтому тестируем
    /// создание структуры ИЗ JSON, а не наоборот.
    #[test]
    fn test_ask_request_deserialization() {
        // Это то, что приходит от клиента
        let json = r#"{"question": "Что такое Rust?"}"#;
        
        // Deserialize: JSON → AskRequest
        let request: AskRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.question, "Что такое Rust?");
    }

    /// Тест СЕРИАЛИЗАЦИИ AskResponse (структура → JSON).
    ///
    /// AskResponse имеет только Serialize, поэтому тестируем
    /// преобразование структуры В JSON.
    #[test]
    fn test_ask_response_serialization() {
        let response = AskResponse {
            answer: "Rust - это язык программирования".to_string(),
            source: "mock".to_string(),
            system_prompt_applied: false,
        };
        
        // Serialize: AskResponse → JSON
        let json = serde_json::to_string(&response).unwrap();
        
        assert!(json.contains("Rust"));
        assert!(json.contains("mock"));
    }

    /// Тест ErrorResponse с кодом и без.
    #[test]
    fn test_error_response_creation() {
        // Без кода
        let error = ErrorResponse::new("Тестовая ошибка");
        assert_eq!(error.error, "Тестовая ошибка");
        assert!(error.code.is_none());

        // С кодом
        let error_with_code = ErrorResponse::with_code("Тестовая ошибка", "TEST_ERROR");
        assert_eq!(error_with_code.code, Some("TEST_ERROR".to_string()));
    }

    /// Тест skip_serializing_if для ErrorResponse.
    ///
    /// Проверяем, что поле code пропускается, когда оно None.
    #[test]
    fn test_error_response_skip_serializing() {
        // Без кода - поле "code" НЕ должно быть в JSON
        let error_no_code = ErrorResponse::new("Ошибка");
        let json = serde_json::to_string(&error_no_code).unwrap();
        assert!(!json.contains("code")); // Поля нет!
        
        // С кодом - поле "code" ДОЛЖНО быть в JSON
        let error_with_code = ErrorResponse::with_code("Ошибка", "ERR_001");
        let json = serde_json::to_string(&error_with_code).unwrap();
        assert!(json.contains("code"));
        assert!(json.contains("ERR_001"));
    }
}
