//! Модуль сервисов для работы с AI.
//!
//! Этот модуль содержит трейт `AiService` и его реализации:
//! - `GigaChatService` - реальная интеграция с GigaChat API
//! - `MockAiService` - заглушка для тестирования и работы без API
//!
//! # Ключевые концепции для изучения
//!
//! ## 1. Trait Objects (Трейт-объекты)
//!
//! В Rust мы используем `Box<dyn AiService>` - это "трейт-объект".
//! Он позволяет хранить РАЗНЫЕ типы, реализующие один трейт, в одной переменной.
//!
//! ```text
//! Без trait objects:
//!   let service: GigaChatService = ...;  // Только один конкретный тип
//!
//! С trait objects:
//!   let service: Box<dyn AiService> = ...; // Любой тип, реализующий AiService
//! ```
//!
//! Это похоже на интерфейсы в Java/C#, но с важным отличием:
//! Rust должен знать размер переменной на этапе компиляции.
//! `dyn AiService` имеет неизвестный размер, поэтому оборачиваем в `Box<>`.
//!
//! ## 2. Send + Sync (Потокобезопасность)
//!
//! - `Send` - тип можно ПЕРЕДАТЬ в другой поток
//! - `Sync` - тип можно ИСПОЛЬЗОВАТЬ из нескольких потоков одновременно
//!
//! Rocket - асинхронный веб-сервер. Он обрабатывает запросы в разных потоках.
//! Поэтому всё, что мы передаём в Rocket через `.manage()`, должно быть Send + Sync.
//!
//! ## 3. async_trait
//!
//! В стандартном Rust нельзя писать `async fn` в трейтах напрямую.
//! Макрос `#[async_trait]` решает эту проблему, преобразуя async-методы
//! в обычные методы, возвращающие `Pin<Box<dyn Future>>`.

// ============================================================================
// ИМПОРТЫ
// ============================================================================

// async_trait - макрос, позволяющий использовать async fn в трейтах.
// Без него Rust не позволит объявить асинхронные методы в trait.
use async_trait::async_trait;

// thiserror - удобный макрос для создания кастомных типов ошибок.
// Автоматически реализует std::error::Error и Display.
use thiserror::Error;

#[cfg(feature = "gigachat")]
use gigalib::controllers::{
    chat::Chat,
    client::ClientBuilder,
};

use crate::config::GigaChatConfig;

// ============================================================================
// ТИПЫ ОШИБОК
// ============================================================================

/// Ошибки, которые могут возникнуть при работе с AI сервисом.
///
/// # Для студентов: Паттерн "Кастомные ошибки"
///
/// В Rust принято создавать свой enum для ошибок каждого модуля.
/// Макрос `#[derive(Error)]` из библиотеки `thiserror` автоматически:
/// - Реализует трейт `std::error::Error`
/// - Реализует трейт `Display` (используя текст из `#[error("...")]`)
///
/// Это позволяет:
/// 1. Точно знать, какие ошибки может вернуть функция
/// 2. Обрабатывать разные ошибки по-разному через `match`
/// 3. Использовать оператор `?` для автоматического преобразования ошибок
#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum AiServiceError {
    /// Ошибка при обращении к API (сеть, таймаут, неверный ответ)
    #[error("Ошибка API: {0}")]
    ApiError(String),

    /// Ошибка конфигурации (отсутствует токен, неверные параметры)
    #[error("Ошибка конфигурации: {0}")]
    ConfigError(String),

    /// Внутренняя ошибка (проблемы с потоками, паника)
    #[error("Внутренняя ошибка: {0}")]
    InternalError(String),
}

// ============================================================================
// ТРЕЙТ AI СЕРВИСА
// ============================================================================

/// Трейт для работы с AI сервисами.
///
/// # Для студентов: Паттерн "Стратегия" (Strategy Pattern)
///
/// Этот трейт - пример паттерна "Стратегия". Он определяет ИНТЕРФЕЙС,
/// а конкретные реализации (GigaChatService, MockAiService) предоставляют
/// разное ПОВЕДЕНИЕ.
///
/// Преимущества:
/// - Код обработчиков не знает, какой именно сервис используется
/// - Легко добавить новый AI-провайдер (OpenAI, Anthropic и т.д.)
/// - Удобно тестировать с mock-реализацией
///
/// # Разбор объявления трейта
///
/// ```text
/// #[async_trait]              // Макрос, разрешающий async fn в трейте
/// pub trait AiService:        // Объявляем публичный трейт с именем AiService
///     Send +                  // Можно передавать между потоками
///     Sync                    // Можно использовать из нескольких потоков
/// ```
///
/// ## Почему Send + Sync?
///
/// Rocket обрабатывает HTTP-запросы параллельно в разных потоках.
/// Наш сервис хранится в `State<>` и доступен всем обработчикам.
/// Без `Send + Sync` Rust не позволит использовать сервис в многопоточном контексте.
///
/// # Примеры использования
///
/// ```rust,ignore
/// // Функция принимает ЛЮБОЙ тип, реализующий AiService
/// async fn process(service: &dyn AiService, q: &str) -> Result<String, AiServiceError> {
///     service.ask(q).await
/// }
/// ```
#[async_trait]
pub trait AiService: Send + Sync {
    /// Отправляет вопрос в AI и получает ответ.
    ///
    /// # Аргументы
    ///
    /// * `question` - Вопрос пользователя
    ///
    /// # Возвращает
    ///
    /// Ответ от AI в виде строки.
    ///
    /// # Ошибки
    ///
    /// Возвращает `AiServiceError` при ошибке обращения к API.
    async fn ask(&self, question: &str) -> Result<String, AiServiceError>;

    /// Возвращает имя сервиса.
    ///
    /// # Примеры
    ///
    /// ```rust
    /// use rust_gigachat_demo::services::{AiService, MockAiService};
    ///
    /// let service = MockAiService::new();
    /// let name = service.name();
    /// println!("Используется сервис: {}", name);
    /// ```
    fn name(&self) -> &str;

    /// Применён ли системный промпт к запросам этого сервиса.
    fn system_prompt_applied(&self) -> bool;
}

// ============================================================================
// РЕАЛИЗАЦИЯ GIGACHAT СЕРВИСА
// ============================================================================

/// Реализация AI сервиса с использованием GigaChat API.
///
/// # Для студентов: Условная компиляция
///
/// Атрибут `#[cfg(feature = "gigachat")]` означает:
/// "Компилировать этот код ТОЛЬКО если включена фича gigachat в Cargo.toml"
///
/// Это позволяет:
/// - Уменьшить размер бинарника, если GigaChat не нужен
/// - Избежать установки зависимостей gigalib
/// - Собрать проект даже без доступа к GigaChat API
///
/// Включение фичи в Cargo.toml:
/// ```toml
/// [features]
/// default = ["gigachat"]  # Включена по умолчанию
/// gigachat = ["gigalib"]  # Подключает библиотеку gigalib
/// ```
#[cfg(feature = "gigachat")]
pub struct GigaChatService {
    /// Токен авторизации для GigaChat API
    token: String,
    
    /// Конфигурация (модель, температура, max_tokens)
    config: GigaChatConfig,

    /// Системный промпт для модели (может быть пустым).
    system_prompt: Option<String>,
}

#[cfg(feature = "gigachat")]
impl GigaChatService {
    /// Создаёт новый экземпляр `GigaChatService`.
    ///
    /// # Аргументы
    ///
    /// * `token` - Токен авторизации GigaChat API
    /// * `config` - Конфигурация GigaChat
    ///
    /// # Примеры
    ///
    /// ```rust
    /// use rust_gigachat_demo::config::GigaChatConfig;
    /// use rust_gigachat_demo::services::GigaChatService;
    ///
    /// let config = GigaChatConfig {
    ///     enabled: true,
    ///     model: "GigaChat".to_string(),
    ///     max_tokens: 128,
    ///     temperature: 0.7,
    ///     timeout_seconds: 30,
    /// };
    /// let token = "TOKEN".to_string();
    /// let _service = GigaChatService::new(token, config, None);
    /// ```
    pub fn new(token: String, config: GigaChatConfig, system_prompt: Option<String>) -> Self {
        Self { 
            token, 
            config,
            system_prompt,
        }
    }
}

#[cfg(feature = "gigachat")]
#[async_trait]
impl AiService for GigaChatService {
    /// Отправляет вопрос в GigaChat API и возвращает ответ.
    ///
    /// # Для студентов: Сложная асинхронная архитектура
    ///
    /// Здесь используется продвинутая техника `spawn_blocking`.
    /// Разберём, почему это необходимо:
    ///
    /// ## Проблема
    ///
    /// Библиотека `gigalib` внутри использует типы, которые НЕ являются `Send`.
    /// Это значит, что их нельзя использовать напрямую в async-контексте Rocket,
    /// где задачи могут переключаться между потоками.
    ///
    /// ## Решение: spawn_blocking
    ///
    /// `tokio::task::spawn_blocking` создаёт ОТДЕЛЬНЫЙ поток, в котором:
    /// 1. Создаётся клиент GigaChat (не Send)
    /// 2. Выполняется запрос к API
    /// 3. Результат возвращается в основной async-контекст
    ///
    /// ## Схема выполнения
    ///
    /// ```text
    /// [Rocket async] --spawn_blocking--> [Blocking thread]
    ///       |                                   |
    ///       |  (ожидает)                       создаёт GigaClient
    ///       |                                   |
    ///       |                                  отправляет запрос
    ///       |                                   |
    ///       <------ результат -------------------|
    /// ```
    async fn ask(&self, question: &str) -> Result<String, AiServiceError> {
        if self.token.trim().is_empty() {
            return Err(AiServiceError::ConfigError(
                "GigaChat token is empty".to_string(),
            ));
        }

        // Клонируем данные, чтобы передать их в другой поток.
        // `move` в замыкании забирает владение, поэтому нужны копии.
        let token = self.token.clone();
        let config = self.config.clone();
        let system_prompt = self
            .system_prompt
            .as_ref()
            .map(|prompt| prompt.trim().to_string())
            .filter(|prompt| !prompt.is_empty());
        let question = question.to_string();
        let prompt = if let Some(prompt) = system_prompt {
            format!(
                "Системные инструкции (не выводи пользователю):\n{prompt}\n\nВопрос пользователя:\n{question}"
            )
        } else {
            question
        };
        
        // spawn_blocking запускает замыкание в отдельном потоке,
        // предназначенном для блокирующих операций.
        // Это НЕ блокирует async runtime Rocket.
        let result = tokio::task::spawn_blocking(move || {
            use gigalib::http::message::MessageConfigBuilder;
            
            // Внутри blocking-потока создаём клиента.
            // Здесь GigaClient безопасен, т.к. мы в обычном (не async) контексте.
            let msg_config = MessageConfigBuilder::new()
                .set_max_tokens(config.max_tokens)
                .set_model(&config.model)
                .set_temp(config.temperature)
                .build();

            let client = ClientBuilder::new()
                .set_basic_token(&token)
                .set_msg_cfg(msg_config)
                .build();
            
            let mut chat = Chat::new(client);
            
            // gigalib требует async для send_message, поэтому создаём
            // локальный runtime внутри blocking-потока.
            // Это не идеально, но необходимо из-за архитектуры gigalib.
            let runtime = tokio::runtime::Runtime::new().unwrap();
            
            runtime.block_on(async {
                chat.send_message(prompt.into())
                    .await
                    .map(|resp| resp.content)
            })
        })
        .await
        // Первый ? - ошибка spawn_blocking (паника в потоке)
        .map_err(|e| AiServiceError::InternalError(e.to_string()))?
        // Второй ? - ошибка от gigalib (сеть, API)
        .map_err(|e| AiServiceError::ApiError(e.to_string()))?;

        Ok(result)
    }

    fn name(&self) -> &str {
        "GigaChat"
    }

    fn system_prompt_applied(&self) -> bool {
        self.system_prompt
            .as_ref()
            .map(|prompt| !prompt.trim().is_empty())
            .unwrap_or(false)
    }
}

// ============================================================================
// MOCK РЕАЛИЗАЦИЯ (ЗАГЛУШКА)
// ============================================================================

/// Mock-реализация AI сервиса для тестирования.
///
/// # Для студентов: Паттерн "Mock Object"
///
/// Mock (заглушка) - это объект, имитирующий поведение реального компонента.
/// Используется для:
///
/// 1. **Разработки без внешних зависимостей**
///    - Не нужен токен GigaChat
///    - Не нужен интернет
///    - Мгновенные ответы (без задержки API)
///
/// 2. **Тестирования**
///    - Предсказуемые ответы
///    - Можно проверить edge cases
///    - Быстрое выполнение тестов
///
/// 3. **Демонстрации**
///    - Показать работу приложения без реального API
///    - Полезно для презентаций и лабораторных работ
///
/// # Реализация
///
/// `MockAiService` - это unit struct (структура без полей).
/// Она не хранит состояния, просто предоставляет методы.
pub struct MockAiService;

impl MockAiService {
    /// Создаёт новый экземпляр `MockAiService`.
    ///
    /// # Примеры
    ///
    /// ```rust
    /// use rust_gigachat_demo::services::MockAiService;
    ///
    /// let service = MockAiService::new();
    /// ```
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockAiService {
    fn default() -> Self {
        Self::new()
    }
}

/// Маркер начала блока «выжимка» в структурированном mock-ответе для `/summarize`.
pub const MOCK_SUMMARIZE_SUMMARY_TAG: &str = "<<<SUMMARY>>>";
/// Маркер начала списка ключевых терминов.
pub const MOCK_SUMMARIZE_TERMS_TAG: &str = "<<<TERMS>>>";

/// Префикс промпта, по которому MockAiService узнаёт «конспектора».
/// Должен совпадать с тем, что формирует `handlers::summarize`.
const SUMMARIZE_PROMPT_PREFIX: &str = "Сделай краткий конспект";

/// Строит структурированный mock-ответ для запроса конспектирования.
///
/// На вход принимается полный текст промпта, который формирует
/// `handlers::summarize`. Функция извлекает из него:
/// - желаемый максимум ключевых терминов («до N»),
/// - исходный текст (всё, что после маркера «Текст:\n\n»),
/// а затем считает простую «выжимку» (первое предложение) и список
/// ключевых терминов (частотный анализ слов длиной ≥ 5 символов).
///
/// Возвращает строку в формате:
/// ```text
/// <<<SUMMARY>>>
/// Первое предложение текста.
/// <<<TERMS>>>
/// term1
/// term2
/// term3
/// ```
/// Handler разбирает её обратно в `SummarizeResponse`.
fn build_mock_summary_payload(prompt: &str) -> String {
    let max_terms = extract_max_terms(prompt).unwrap_or(5);
    let text = extract_source_text(prompt);

    let summary = first_sentence(text).unwrap_or_else(|| text.trim().to_string());
    let terms = top_frequent_terms(text, max_terms);

    let mut out = String::with_capacity(text.len() / 4 + 64);
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

/// Извлекает число после фрагмента «до » в промпте (например, «до 5 ключевых …»).
fn extract_max_terms(prompt: &str) -> Option<usize> {
    let idx = prompt.find("до ")?;
    let after = &prompt[idx + "до ".len()..];
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Извлекает «полезный» текст из промпта — всё, что после маркера «Текст:».
fn extract_source_text(prompt: &str) -> &str {
    if let Some(idx) = prompt.find("Текст:") {
        let after = &prompt[idx + "Текст:".len()..];
        after.trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ')
    } else {
        prompt
    }
}

/// Возвращает первое предложение текста (до `.`, `!`, `?`), включая знак,
/// если оно непустое.
fn first_sentence(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    for (i, ch) in trimmed.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            let end = i + ch.len_utf8();
            let candidate = trimmed[..end].trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    Some(trimmed.trim().to_string())
}

/// Топ-N слов по частоте (длина ≥ 5 символов, регистр не учитывается,
/// пунктуация отбрасывается). При равной частоте — лексикографический
/// порядок, чтобы вывод был детерминирован.
fn top_frequent_terms(text: &str, max_terms: usize) -> Vec<String> {
    use std::collections::HashMap;

    if max_terms == 0 {
        return Vec::new();
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for raw in text.split_whitespace() {
        let word: String = raw
            .chars()
            .filter(|c| c.is_alphabetic())
            .collect::<String>()
            .to_lowercase();
        if word.chars().count() >= 5 {
            *counts.entry(word).or_insert(0) += 1;
        }
    }

    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs.into_iter().take(max_terms).map(|(w, _)| w).collect()
}

#[async_trait]
impl AiService for MockAiService {
    async fn ask(&self, question: &str) -> Result<String, AiServiceError> {
        // Специальная ветка: handler /summarize формирует промпт, который
        // начинается с конкретного маркера. На этот класс запросов отвечаем
        // структурированным mock-результатом, чтобы handler мог разобрать
        // выжимку и список ключевых терминов независимо от языка модели.
        if question.starts_with(SUMMARIZE_PROMPT_PREFIX) {
            return Ok(build_mock_summary_payload(question));
        }

        // Return mock response based on question keywords
        let question_lower = question.to_lowercase();

        // Check more specific topics BEFORE general "rust"
        // Note: Use word boundaries - "hi" should not match "this"
        let is_greeting = question_lower.contains("hello")
            || question_lower.starts_with("hi ")
            || question_lower.starts_with("hi!")
            || question_lower.starts_with("hi,")
            || question_lower == "hi";
        
        let answer = if is_greeting {
            "Hello! I'm a demo AI assistant for the Rust project.\n\n\
             I'm running in mock mode, but I can answer questions about:\n\
             - Rust programming language\n\
             - Rocket web framework\n\
             - Async programming\n\
             - REST API and JSON\n\
             - Testing\n\
             - Error handling\n\n\
             Try asking me about any of these topics! For full AI capabilities, \
             configure the GigaChat API connection."
        } else if question_lower.contains("rocket") {
            "Rocket is a web framework for Rust that makes building fast and secure \
             web applications simple and enjoyable. Key features:\n\
             - Compile-time type safety\n\
             - Convenient routing macros (#[get], #[post], etc.)\n\
             - Automatic JSON deserialization\n\
             - Built-in testing support\n\
             - Flexible middleware system (fairings)\n\
             Rocket is ideal for building REST APIs and web services."
        } else if question_lower.contains("test") {
            "Testing in Rust is a built-in language feature. Types of tests:\n\
             - Unit tests (#[test]) - test individual functions\n\
             - Integration tests (tests/ folder) - test component interactions\n\
             - Doc tests - examples in documentation that are automatically verified\n\
             Rocket provides convenient tools for testing web apps via \
             rocket::local::blocking::Client. Run with: cargo test"
        } else if question_lower.contains("error") {
            "Error handling in Rust is based on Result<T, E> and Option<T> types:\n\
             - Result - for operations that may fail\n\
             - Option - for values that may be absent\n\
             - ? operator - for convenient error propagation\n\
             - thiserror - library for creating custom error types\n\
             This approach forces explicit error handling and eliminates many runtime issues."
        } else if question_lower.contains("serde") || question_lower.contains("json") {
            "Serde is a powerful framework for serializing and deserializing data in Rust. \
             It allows you to:\n\
             - Automatically convert JSON to Rust structs\n\
             - Convert structs back to JSON\n\
             - Work with other formats (TOML, YAML, MessagePack)\n\
             - Use derive macros for automatic code generation\n\
             Example: #[derive(Serialize, Deserialize)] makes a struct JSON-compatible."
        } else if question_lower.contains("async") {
            "Async programming in Rust allows efficient handling of many tasks \
             simultaneously without creating many threads. Key concepts:\n\
             - async/await - syntax for async functions\n\
             - Future - trait for async computations\n\
             - Tokio - popular async runtime\n\
             - Async trait - for async methods in traits\n\
             Especially useful for web servers, network apps, and I/O operations."
        } else if question_lower.contains("api") {
            "REST API (Representational State Transfer) is an architectural style for \
             building web services. Main principles:\n\
             - GET - retrieve data\n\
             - POST - create new resources\n\
             - PUT/PATCH - update existing resources\n\
             - DELETE - remove resources\n\
             With Rust and Rocket, building APIs is convenient thanks to type safety \
             and automatic JSON handling via serde."
        } else if question_lower.contains("how") && question_lower.contains("work") {
            "This app is a demo project showing how to build a web service in Rust. \
             Architecture:\n\
             - Rocket - accepts HTTP requests\n\
             - Handlers - process requests (in src/handlers/)\n\
             - Services - business logic and AI integration (in src/services/)\n\
             - Models - data structures for API (in src/models/)\n\
             - Config - configuration management (config.toml)\n\n\
             The service can run in two modes: with real GigaChat API or with mocks (current)."
        } else if question_lower.contains("rust") {
            "Rust is a systems programming language focused on safety, speed, and concurrency. \
             It was developed by Mozilla Research and first released in 2010. \
             Rust guarantees memory safety without using a garbage collector through its \
             ownership and borrowing system. This makes Rust ideal for systems programming, \
             web servers, embedded systems, and high-performance applications."
        } else {
            "This is a demo response from the mock service.\n\n\
             I can help with questions about:\n\
             - Rust and its features\n\
             - Rocket web framework\n\
             - Async programming\n\
             - REST API\n\
             - Testing\n\n\
             Try asking: 'What is Rust?' or 'How does Rocket work?'\n\n\
             For real AI responses, configure the GigaChat API by setting \
             GIGACHAT_TOKEN environment variable and gigachat.enabled=true in config.toml."
        };

        Ok(answer.to_string())
    }

    fn name(&self) -> &str {
        "Mock AI Service"
    }

    fn system_prompt_applied(&self) -> bool {
        false
    }
}

// ============================================================================
// ФАБРИКА СЕРВИСОВ
// ============================================================================

/// Фабрика для создания AI сервисов.
///
/// # Для студентов: Паттерн "Фабрика" (Factory Pattern)
///
/// Фабрика - это паттерн, который ИНКАПСУЛИРУЕТ логику создания объектов.
/// Вместо того чтобы создавать объекты напрямую:
///
/// ```rust,ignore
/// // Плохо: логика выбора размазана по коду
/// let service = if config.enabled && token.is_some() {
///     Box::new(GigaChatService::new(...))
/// } else {
///     Box::new(MockAiService::new())
/// };
/// ```
///
/// Мы используем фабрику:
///
/// ```rust,ignore
/// // Хорошо: логика выбора в одном месте
/// let service = AiServiceFactory::create(&config, token);
/// ```
///
/// ## Преимущества
///
/// 1. **Единая точка создания** - логика в одном месте
/// 2. **Легко добавить новые типы** - только изменить фабрику
/// 3. **Упрощает тестирование** - можно подменить фабрику
/// 4. **Скрывает сложность** - вызывающий код не знает деталей
pub struct AiServiceFactory;

impl AiServiceFactory {
    /// Создаёт AI сервис на основе конфигурации.
    ///
    /// # Для студентов: Возвращаемый тип `Box<dyn AiService>`
    ///
    /// Почему `Box<dyn AiService>`, а не просто `impl AiService`?
    ///
    /// 1. **`impl AiService`** - компилятор должен знать КОНКРЕТНЫЙ тип на этапе компиляции.
    ///    Но мы возвращаем РАЗНЫЕ типы в зависимости от условия!
    ///
    /// 2. **`Box<dyn AiService>`** - это trait object. Конкретный тип определяется
    ///    во время ВЫПОЛНЕНИЯ программы (runtime).
    ///
    /// ```text
    /// Box<dyn AiService>
    /// ^^^  ^^^  ^^^^^^^^^
    ///  |    |       |
    ///  |    |       +-- Любой тип, реализующий AiService
    ///  |    +---------- "dynamic" - тип определяется в runtime
    ///  +--------------- Умный указатель, хранит объект в куче (heap)
    /// ```
    ///
    /// # Логика выбора
    ///
    /// - Если `enabled=true` И есть токен → GigaChatService
    /// - Иначе → MockAiService
    #[cfg(feature = "gigachat")]
    pub fn create(
        config: &GigaChatConfig,
        token: Option<String>,
        system_prompt: Option<String>,
    ) -> Box<dyn AiService> {
        match (config.enabled, token) {
            (true, Some(token)) => {
                Box::new(GigaChatService::new(token, config.clone(), system_prompt))
            }
            _ => Box::new(MockAiService::new()),
        }
    }

    /// Версия без фичи gigachat - всегда возвращает MockAiService.
    ///
    /// # Для студентов: Зачем две версии метода?
    ///
    /// Атрибуты `#[cfg(...)]` позволяют иметь разные реализации
    /// одного метода для разных конфигураций сборки.
    ///
    /// - `#[cfg(feature = "gigachat")]` - код компилируется ЕСЛИ фича включена
    /// - `#[cfg(not(feature = "gigachat"))]` - код компилируется ЕСЛИ фича ВЫКЛЮЧЕНА
    ///
    /// Параметры с `_` (`_config`, `_token`) означают, что они не используются,
    /// но нужны для совместимости сигнатуры метода.
    #[cfg(not(feature = "gigachat"))]
    pub fn create(
        _config: &GigaChatConfig,
        _token: Option<String>,
        _system_prompt: Option<String>,
    ) -> Box<dyn AiService> {
        Box::new(MockAiService::new())
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

/// # Для студентов: Атрибут `#[cfg(test)]`
///
/// `#[cfg(test)]` - это условная компиляция. Код внутри компилируется
/// ТОЛЬКО при запуске тестов (`cargo test`).
///
/// ```text
/// cargo build  →  mod tests НЕ компилируется (экономия времени/размера)
/// cargo test   →  mod tests компилируется и запускается
/// ```
///
/// Это стандартная практика: тесты живут рядом с кодом, но не попадают в релиз.
#[cfg(test)]
mod tests {
    // `use super::*` импортирует всё из родительского модуля (services)
    use super::*;

    /// # Для студентов: `#[tokio::test]` vs `#[test]`
    ///
    /// ```text
    /// #[test]         - для СИНХРОННЫХ тестов (обычные функции)
    /// #[tokio::test]  - для АСИНХРОННЫХ тестов (async fn)
    /// ```
    ///
    /// Наш метод `ask()` - асинхронный (`async fn`), поэтому:
    /// - Тест тоже должен быть `async fn`
    /// - Нужен async runtime для выполнения
    /// - `#[tokio::test]` создаёт этот runtime автоматически
    ///
    /// ## Что делает `#[tokio::test]`?
    ///
    /// Преобразует:
    /// ```rust,ignore
    /// #[tokio::test]
    /// async fn my_test() { ... }
    /// ```
    ///
    /// В эквивалент:
    /// ```rust,ignore
    /// #[test]
    /// fn my_test() {
    ///     tokio::runtime::Runtime::new()
    ///         .unwrap()
    ///         .block_on(async { ... })
    /// }
    /// ```
    #[tokio::test]
    async fn test_mock_service() {
        let service = MockAiService::new();
        // .await - ждём завершения асинхронной операции
        let answer = service.ask("Что такое Rust?").await.unwrap();
        assert!(answer.contains("Rust"));
    }

    #[tokio::test]
    async fn test_mock_service_name() {
        let service = MockAiService::new();
        assert_eq!(service.name(), "Mock AI Service");
    }
}
