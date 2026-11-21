use iced::widget::{button, column, container, row, text, vertical_space};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Instances,
    Catalog,
    Modpacks,
    Downloads,
    Accounts,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformChoice {
    Java,
    Horizon,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    SelectPlatform(PlatformChoice),
    Play,
    CreateInstance,
}

#[derive(Debug)]
pub struct App {
    page: Page,
    platform: PlatformChoice,
    notice: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            page: Page::Home,
            platform: PlatformChoice::Java,
            notice: None,
        }
    }
}

pub fn update(app: &mut App, message: Message) {
    match message {
        Message::Navigate(page) => {
            app.page = page;
            app.notice = None;
        }
        Message::SelectPlatform(platform) => {
            app.platform = platform;
            app.notice = Some(match platform {
                PlatformChoice::Java => "Выбрана платформа Minecraft Java".into(),
                PlatformChoice::Horizon => "Выбрана платформа Horizon".into(),
            });
        }
        Message::Play => {
            app.notice = Some("Игровой запуск будет подключён на следующем этапе.".into());
        }
        Message::CreateInstance => {
            app.page = Page::Instances;
            app.notice = Some("Менеджер игровых экземпляров пока находится в разработке.".into());
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let sidebar = sidebar(app);
    let content = page_content(app);

    row![
        container(sidebar)
            .width(Length::Fixed(248.0))
            .height(Length::Fill)
            .padding(18)
            .style(container::dark),
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(28),
    ]
    .spacing(0)
    .height(Length::Fill)
    .into()
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let nav = column![
        nav_button("⌂   Обзор", Page::Home, app.page),
        nav_button("▦   Мои сборки", Page::Instances, app.page),
        nav_button("◇   Каталог модов", Page::Catalog, app.page),
        nav_button("▤   Модпаки", Page::Modpacks, app.page),
        nav_button("⇩   Загрузки", Page::Downloads, app.page),
        nav_button("♙   Аккаунты", Page::Accounts, app.page),
    ]
    .spacing(7);

    column![
        row![
            text("✦").size(28).color(iced::Color::from_rgb8(104, 207, 218)),
            column![
                text("GLOWSTONECL").size(18),
                text("UNIVERSAL LAUNCHER").size(9),
            ]
            .spacing(0),
        ]
        .align_y(Alignment::Center)
        .spacing(8),
        vertical_space().height(24),
        text("РАЗДЕЛЫ").size(10),
        nav,
        vertical_space().height(Length::Fill),
        text("EARLY DEVELOPMENT · 0.1.0").size(9),
        button("⚙   Настройки")
            .on_press(Message::Navigate(Page::Settings))
            .width(Length::Fill),
    ]
    .spacing(12)
    .height(Length::Fill)
    .into()
}

fn nav_button(label: &'static str, page: Page, current: Page) -> Element<'static, Message> {
    let label = if page == current {
        format!("▸  {label}")
    } else {
        label.to_owned()
    };

    button(text(label).size(14))
        .on_press(Message::Navigate(page))
        .width(Length::Fill)
        .into()
}

fn page_content(app: &App) -> Element<'_, Message> {
    match app.page {
        Page::Home => home_page(app),
        Page::Instances => empty_page(
            "Мои сборки",
            "Каждый экземпляр будет иметь собственную версию игры, моды, настройки и папку миров.",
            "Пока нет игровых экземпляров",
            "Создать экземпляр",
            Message::CreateInstance,
        ),
        Page::Catalog => empty_page(
            "Каталог модов",
            "Единый поиск по Modrinth, CurseForge и отдельному каталогу модов Horizon.",
            "Каталоги ещё не подключены",
            "Вернуться на главную",
            Message::Navigate(Page::Home),
        ),
        Page::Modpacks => empty_page(
            "Модпаки",
            "Импорт и экспорт Modrinth-паков, CurseForge-паков и сборок Horizon.",
            "Библиотека модпаков пока пуста",
            "Создать экземпляр",
            Message::CreateInstance,
        ),
        Page::Downloads => empty_page(
            "Загрузки",
            "Очередь загрузок, скорость, проверка контрольных сумм и повторные попытки.",
            "Нет активных загрузок",
            "На главную",
            Message::Navigate(Page::Home),
        ),
        Page::Accounts => empty_page(
            "Аккаунты",
            "Microsoft OAuth и локальные offline-профили с защищённым хранением токенов.",
            "Аккаунты не добавлены",
            "На главную",
            Message::Navigate(Page::Home),
        ),
        Page::Settings => settings_page(),
    }
}

fn home_page(app: &App) -> Element<'_, Message> {
    let profile = match app.platform {
        PlatformChoice::Java => ("Vanilla · Java Edition", "MINECRAFT JAVA"),
        PlatformChoice::Horizon => ("Horizon · Stable", "HORIZON · RUST EDITION"),
    };

    let mut content = column![
        text("Добро пожаловать в GlowstoneCL").size(28),
        text("Один лаунчер для Minecraft Java и Horizon.").size(14),
        vertical_space().height(8),
        text("ИГРОВАЯ ПЛАТФОРМА").size(10),
        row![
            button("Minecraft Java")
                .on_press(Message::SelectPlatform(PlatformChoice::Java)),
            button("Horizon")
                .on_press(Message::SelectPlatform(PlatformChoice::Horizon)),
        ]
        .spacing(8),
        container(
            column![
                text("ВЫБРАННЫЙ ПРОФИЛЬ").size(10),
                text(profile.0).size(25),
                text(profile.1)
                    .size(11)
                    .color(iced::Color::from_rgb8(105, 205, 216)),
                text("Профиль появится после подключения менеджера экземпляров.").size(12),
                row![
                    button("▶   Играть").on_press(Message::Play),
                    button("Настроить экземпляр").on_press(Message::CreateInstance),
                ]
                .spacing(10),
            ]
            .spacing(12),
        )
        .padding(22)
        .width(Length::Fill)
        .style(container::rounded_box),
        text("ПЛАТФОРМЫ").size(10),
        row![
            info_card(
                "JAVA EDITION",
                "Vanilla, Forge, NeoForge, Fabric и Quilt. Отдельная JVM и Java-моды.",
            ),
            info_card(
                "HORIZON",
                "Собственные .hmod и .hnmod, отдельная система модов и совместимости.",
            ),
        ]
        .spacing(14)
        .width(Length::Fill),
    ]
    .spacing(16)
    .width(Length::Fill);

    if let Some(notice) = &app.notice {
        content = content.push(
            container(text(notice).size(12))
                .padding(12)
                .width(Length::Fill)
                .style(container::secondary),
        );
    }

    content.into()
}

fn info_card(title: &'static str, description: &'static str) -> Element<'static, Message> {
    container(
        column![
            text(title).size(11).color(iced::Color::from_rgb8(105, 205, 216)),
            text(description).size(12),
        ]
        .spacing(10),
    )
    .padding(17)
    .width(Length::Fill)
    .style(container::rounded_box)
    .into()
}

fn empty_page(
    title: &'static str,
    description: &'static str,
    empty_title: &'static str,
    action_label: &'static str,
    action: Message,
) -> Element<'static, Message> {
    column![
        text(title).size(27),
        text(description).size(13),
        vertical_space().height(20),
        container(
            column![
                text(empty_title).size(18),
                text("Раздел будет подключён в следующих этапах разработки.").size(12),
                button(action_label).on_press(action),
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .padding(30)
        .width(Length::Fill)
        .style(container::rounded_box),
    ]
    .spacing(12)
    .into()
}

fn settings_page() -> Element<'static, Message> {
    #[cfg(target_os = "linux")]
    let backend_info = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        "Обнаружена Wayland-сессия. Доступен нативный Wayland backend."
    } else if std::env::var_os("DISPLAY").is_some() {
        "Обнаружена X11-сессия. Доступен X11 backend."
    } else {
        "Графическая сессия не обнаружена в переменных окружения."
    };

    #[cfg(not(target_os = "linux"))]
    let backend_info = "Используется системный оконный backend.";

    column![
        text("Настройки").size(27),
        text("Параметры лаунчера и игровой среды.").size(13),
        vertical_space().height(12),
        container(
            column![
                text("ОКОННАЯ СИСТЕМА LINUX").size(10),
                text("Нативный Wayland и X11").size(18),
                text(backend_info).size(12),
                text("glowstonecl --backend wayland").size(12),
                text("glowstonecl --backend x11").size(12),
            ]
            .spacing(9),
        )
        .padding(18)
        .width(Length::Fill)
        .style(container::rounded_box),
    ]
    .spacing(12)
    .into()
}
