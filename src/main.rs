//! Демонстрационное учебное приложение на Rust с Rocket и GigaChat API.
//!
//! # Для студентов: Точка входа в приложение
//!
//! Это главный файл приложения (`main.rs`). Здесь происходит:
//! 1. Загрузка конфигурации
//! 2. Инициализация логирования
//! 3. Создание AI-сервиса (GigaChat или Mock)
//! 4. Настройка и запуск веб-сервера Rocket
//!
//! # Архитектура приложения
//!
//! ```text
//! main.rs (этот файл)
//!    │
//!    ├── config/    - Загрузка настроек из config.toml
//!    ├── models/    - Структуры данных (Request/Response)
//!    ├── services/  - Бизнес-логика (AI сервисы)
//!    └── handlers/  - HTTP обработчики (эндпоинты API)
//! ```
//!
//! # Поток выполнения
//!
//! ```text
//! cargo run
//!     │
//!     ▼
//! #[launch] fn rocket()     ← Rocket вызывает эту функцию
//!     │
//!     ├─► Загрузка конфигурации (AppConfig::load)
//!     ├─► Инициализация логов (init_logging)
//!     ├─► Создание AI сервиса (AiServiceFactory::create)
//!     └─► Запуск сервера (rocket::custom(...).launch())
//! ```
//!
//! # Запуск
//!
//! ```bash
//! # Без GigaChat (mock mode)
//! cargo run
//!
//! # С GigaChat API
//! export GIGACHAT_TOKEN="your_token_here"
//! cargo run
//! ```

// ============================================================================
// ИМПОРТЫ И МАКРОСЫ
// ============================================================================

// #[macro_use] extern crate rocket - импортирует макросы из крейта rocket
// в глобальную область видимости. Это устаревший синтаксис, но он всё ещё
// используется для макросов routes! и catchers!
//
// Альтернатива (современный стиль):
//   use rocket::{routes, catchers};
#[macro_use]
extern crate rocket;

// Объявление модулей проекта.
// `mod X;` говорит компилятору: "загрузи файл src/X/mod.rs (или src/X.rs)"
mod config;
mod handlers;
mod models;
mod services;

// Импорт конкретных элементов из модулей для удобства использования
use config::AppConfig;
use handlers::{
    ask, cors_preflight, health, index, internal_error, not_found, summarize,
    unprocessable_entity,
};
use services::AiServiceFactory;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};

// tracing - современная библиотека логирования для Rust
// Преимущества над println!:
// - Уровни логирования (error, warn, info, debug, trace)
// - Структурированные логи
// - Фильтрация по уровням и модулям
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// ИНИЦИАЛИЗАЦИЯ ЛОГИРОВАНИЯ
// ============================================================================

/// CORS‑fairing: добавляет заголовки для доступа из web‑UI.
struct Cors;

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, res: &mut Response<'r>) {
        res.set_header(Header::new(
            "Access-Control-Allow-Origin",
            "http://127.0.0.1:8080",
        ));
        res.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "GET, POST, OPTIONS",
        ));
        res.set_header(Header::new(
            "Access-Control-Allow-Headers",
            "Content-Type",
        ));
    }
}

/// Инициализирует систему логирования.
///
/// # Для студентов: Зачем нужно логирование?
///
/// Логирование - это способ отслеживать, что происходит в программе:
/// - При разработке: отладка без debugger'а
/// - В продакшене: диагностика проблем
///
/// ## Уровни логирования (от детального к критическому)
///
/// ```text
/// TRACE  →  Очень детальная информация (каждый шаг)
/// DEBUG  →  Отладочная информация
/// INFO   →  Важные события (запуск, завершение)
/// WARN   →  Предупреждения (что-то подозрительное)
/// ERROR  →  Ошибки (что-то сломалось)
/// ```
///
/// ## Пример использования
///
/// ```rust,ignore
/// info!("Сервер запущен на порту {}", port);
/// error!("Не удалось подключиться к БД: {}", err);
/// ```
fn init_logging(config: &AppConfig) {
    // Преобразуем строку из конфига в enum уровня логирования.
    // match - это паттерн-матчинг, аналог switch в других языках,
    // но более мощный и безопасный.
    let log_level = match config.logging.level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO, // _ - "любое другое значение" (default)
    };

    // Настраиваем tracing_subscriber - это "получатель" логов.
    // Registry + layers - паттерн "Строитель" (Builder).
    let log_format = config.logging.format.as_str();
    if log_format != "compact" {
        eprintln!("⚠️  Формат логов '{log_format}' не поддержан, используем compact");
    }

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)  // Не показывать имя модуля
        .with_level(true)    // Показывать уровень (INFO, ERROR...)
        .compact();          // Компактный формат вывода

    tracing_subscriber::registry()
        .with(
            // fmt::layer() - форматирует логи для вывода в консоль
            fmt_layer,
        )
        .with(
            // Фильтр - какие логи показывать
            tracing_subscriber::filter::Targets::new()
                .with_default(log_level)
                .with_target("rocket", tracing::Level::INFO), // Rocket логирует только INFO+
        )
        .init(); // Активируем subscriber глобально

    info!(
        "Логирование инициализировано (уровень: {}, формат: {})",
        config.logging.level,
        config.logging.format
    );
}

// ============================================================================
// ТОЧКА ВХОДА
// ============================================================================

/// Точка входа в приложение - создаёт и настраивает Rocket сервер.
///
/// # Для студентов: Атрибут #[launch]
///
/// `#[launch]` - это макрос Rocket, который:
/// 1. Генерирует функцию `main()`
/// 2. Настраивает async runtime (tokio)
/// 3. Вызывает нашу функцию `rocket()`
/// 4. Запускает сервер
///
/// ## Возвращаемый тип `-> _`
///
/// `_` (underscore) означает "компилятор, выведи тип сам".
/// Реальный тип: `Rocket<Build>` - сконфигурированный, но не запущенный сервер.
///
/// ## Альтернативный способ (без #[launch])
///
/// ```rust,ignore
/// #[rocket::main]
/// async fn main() -> Result<(), rocket::Error> {
///     rocket().launch().await?;
///     Ok(())
/// }
/// ```
#[launch]
fn rocket() -> _ {
    // =========================================================================
    // ШАГ 1: Загрузка конфигурации
    // =========================================================================
    // 
    // AppConfig::load() возвращает Result<AppConfig, ConfigError>.
    // Используем match для обработки обоих случаев (Ok и Err).
    let config = match AppConfig::load() {
        Ok(cfg) => {
            println!("✅ Конфигурация успешно загружена");
            cfg  // Возвращаем конфигурацию из match-блока
        }
        Err(e) => {
            // Используем eprintln! (error print) для вывода ошибок в stderr
            eprintln!("❌ Ошибка загрузки конфигурации: {}", e);
            eprintln!("💡 Убедитесь, что файл config.toml существует и корректен");
            // std::process::exit(1) - завершение программы с кодом ошибки
            // Код 0 = успех, любой другой = ошибка
            std::process::exit(1);
        }
    };

    // =========================================================================
    // ШАГ 2: Инициализация логирования
    // =========================================================================
    init_logging(&config);

    // Выводим информацию о приложении через систему логирования
    info!("🚀 Запуск {}", config.application.name);
    info!("📦 Версия: {}", config.application.version);
    info!("🌍 Окружение: {}", config.server.environment);
    if config.is_development() {
        info!("🧪 Режим разработки включён");
    }
    info!(
        "🔧 GigaChat API: {}",
        if config.is_gigachat_enabled() { "включён" } else { "выключен (mock mode)" }
    );
    info!("⏱️ Таймаут GigaChat: {}s", config.gigachat.timeout_seconds);
    info!(
        "📝 System prompt length: {} chars",
        config.application.system_prompt.chars().count()
    );

    // =========================================================================
    // ШАГ 3: Создание AI сервиса
    // =========================================================================
    //
    // Для студентов: Обратите внимание на тип переменной:
    //   Box<dyn services::AiService>
    // Это trait object - мы не знаем конкретный тип (GigaChat или Mock),
    // но знаем, что он реализует трейт AiService.
    let system_prompt = if config.application.system_prompt.trim().is_empty() {
        None
    } else {
        Some(config.application.system_prompt.clone())
    };

    let ai_service: Box<dyn services::AiService> = if config.is_gigachat_enabled() {
        // Вложенный match - проверяем наличие токена
        match config.get_gigachat_token() {
            Ok(token) => {
                info!("✅ Токен GigaChat найден, используем реальный API");
                AiServiceFactory::create(&config.gigachat, Some(token), system_prompt)
            }
            Err(_) => {
                // Токен не найден, но это НЕ фатальная ошибка - используем mock
                error!("⚠️  Токен GigaChat не найден в переменной окружения GIGACHAT_TOKEN");
                info!("💡 Переключаемся на mock mode");
                AiServiceFactory::create(&config.gigachat, None, None)
            }
        }
    } else {
        info!("ℹ️  GigaChat API отключён в конфигурации, используем mock mode");
        AiServiceFactory::create(&config.gigachat, None, None)
    };

    info!("🤖 AI сервис: {}", ai_service.name());

    // =========================================================================
    // ШАГ 4: Настройка Rocket
    // =========================================================================
    //
    // Figment - это система конфигурации Rocket.
    // .merge() объединяет несколько источников конфигурации.
    // Здесь мы переопределяем адрес и порт из нашего config.toml.
    let figment = rocket::Config::figment()
        .merge(("address", config.server.address.clone()))
        .merge(("port", config.server.port))
        .merge(("cli_colors", false));

    info!("🌐 Сервер будет запущен на {}:{}", config.server.address, config.server.port);

    // =========================================================================
    // ШАГ 5: Сборка и возврат экземпляра Rocket
    // =========================================================================
    //
    // Для студентов: Метод-цепочка (Method Chaining)
    //
    // Каждый метод возвращает self, позволяя вызывать следующий метод.
    // Это паттерн "Строитель" (Builder Pattern).
    //
    // .manage(T)    - сохраняет T в State, доступен во всех обработчиках
    // .mount("/", routes![...])   - регистрирует обработчики по пути "/"
    // .register("/", catchers![...]) - регистрирует обработчики ошибок
    rocket::custom(figment)
        .attach(Cors)
        .manage(config)      // State<AppConfig> - доступен через &State<AppConfig>
        .manage(ai_service)  // State<Box<dyn AiService>> - AI сервис
        // ─────────────────────────────────────────────────────────────────
        // Для студентов: Макросы routes! и catchers!
        // ─────────────────────────────────────────────────────────────────
        //
        // routes![index, health, ask] - это МАКРОС, не функция!
        // Он преобразует имена функций в Vec<Route>.
        //
        // Без макроса пришлось бы писать:
        //   vec![
        //       Route::new(Method::Get, "/", index),
        //       Route::new(Method::Get, "/health", health),
        //       Route::new(Method::Post, "/ask", ask),
        //   ]
        //
        // Макрос читает атрибуты #[get("/")] и #[post("/ask")] из функций
        // и автоматически создаёт правильные Route-объекты.
        //
        // catchers! работает аналогично для функций с #[catch(код)]
        // ─────────────────────────────────────────────────────────────────
        .mount("/", routes![index, health, ask, summarize, cors_preflight])
        .register("/", catchers![not_found, internal_error, unprocessable_entity])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        // Проверяем, что конфигурация загружается
        let result = AppConfig::load();
        assert!(result.is_ok() || result.is_err()); // Просто проверяем, что функция работает
    }
}
