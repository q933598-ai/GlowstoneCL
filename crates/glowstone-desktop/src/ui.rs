use iced::widget::{
    button, canvas, column, container, image, row, rule, scrollable, stack, text,
    text_input, Space,
};
use iced::{
    Alignment, Border, Color, ContentFit, Element, Length, Rectangle, Shadow, Vector,
};

const BG: Color = Color::from_rgb8(10, 15, 26);
const SIDEBAR: Color = Color::from_rgba(0.055, 0.085, 0.15, 0.94);
const PANEL: Color = Color::from_rgba(0.09, 0.13, 0.21, 0.88);
const PANEL_LIGHT: Color = Color::from_rgba(0.15, 0.20, 0.31, 0.72);
const TEXT: Color = Color::from_rgb8(242, 246, 252);
const MUTED: Color = Color::from_rgb8(164, 177, 198);
const SUBTLE: Color = Color::from_rgb8(119, 137, 164);
const CYAN: Color = Color::from_rgb8(105, 218, 235);
const CYAN_DARK: Color = Color::from_rgb8(17, 47, 64);
const BLUE: Color = Color::from_rgb8(91, 151, 243);
const PURPLE: Color = Color::from_rgb8(164, 133, 255);
const GREEN: Color = Color::from_rgb8(105, 225, 126);
const ORANGE: Color = Color::from_rgb8(255, 157, 94);
const EDGE: Color = Color::from_rgba(0.76, 0.86, 1.0, 0.16);
const EDGE_BRIGHT: Color = Color::from_rgba(0.43, 0.83, 1.0, 0.58);
const DIVIDER: Color = Color::from_rgba(0.72, 0.82, 0.95, 0.13);

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
    SearchChanged(String),
    Info(&'static str),
}

#[derive(Debug)]
pub struct App {
    page: Page,
    platform: PlatformChoice,
    search: String,
    notice: Option<String>,
    art: image::Handle,
}

impl Default for App {
    fn default() -> Self {
        Self {
            page: Page::Home,
            platform: PlatformChoice::Java,
            search: String::new(),
            notice: None,
            art: image::Handle::from_bytes(&include_bytes!("../assets/hero.jpg")[..]),
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
        Message::SearchChanged(value) => {
            app.search = value;
            app.page = Page::Catalog;
            app.notice = None;
        }
        Message::Info(message) => {
            app.notice = Some(message.to_owned());
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let content = row![
        sidebar(app),
        column![
            top_bar(app),
            row![
                scrollable(main_content(app))
                    .height(Length::Fill)
                    .width(Length::Fill),
                scrollable(right_rail(app))
                    .height(Length::Fill)
                    .width(Length::Fixed(270.0)),
            ]
            .spacing(14)
            .height(Length::Fill)
            .width(Length::Fill),
        ]
        .spacing(14)
        .height(Length::Fill)
        .width(Length::Fill),
    ]
    .spacing(14)
    .height(Length::Fill);

    stack![
        canvas(GlowBackdrop)
            .width(Length::Fill)
            .height(Length::Fill),
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(12),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .into()
}

#[derive(Debug, Clone, Copy)]
struct GlowBackdrop;

impl<Message> canvas::Program<Message> for GlowBackdrop {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(iced::Point::ORIGIN, bounds.size());
        frame.fill(&background, BG);

        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.72, bounds.height * 0.12),
            440.0,
            Color::from_rgb8(45, 89, 174),
        );
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.42, bounds.height * 0.95),
            360.0,
            Color::from_rgb8(81, 61, 157),
        );
        draw_glow(
            &mut frame,
            iced::Point::new(bounds.width * 0.98, bounds.height * 0.72),
            280.0,
            Color::from_rgb8(38, 121, 166),
        );

        vec![frame.into_geometry()]
    }
}

fn draw_glow(
    frame: &mut canvas::Frame,
    center: iced::Point,
    radius: f32,
    color: Color,
) {
    for layer in 0..24 {
        let amount = layer as f32 / 23.0;
        let current_radius = radius * (1.0 - amount * 0.94);
        let alpha = 0.001 + (1.0 - amount).powf(2.0) * 0.008;
        let circle = canvas::Path::circle(center, current_radius);
        frame.fill(
            &circle,
            Color::from_rgba(color.r, color.g, color.b, alpha),
        );
    }
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let brand = container(
        row![
            container(text("✦").size(24).color(CYAN))
                .width(48)
                .height(48)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT)),
            column![
                text("GlowstoneCL").size(16).color(TEXT),
                text("UNIVERSAL LAUNCHER").size(8).color(CYAN),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center)
        .spacing(10),
    )
    .padding(12)
    .width(Length::Fill)
    .style(|_| glass_style(Color::from_rgba(0.08, 0.12, 0.21, 0.68), EDGE));

    let library = column![
        section_label("БИБЛИОТЕКА"),
        nav_button("⌂", "Главная", Page::Home, app.page),
        nav_button("▦", "Сборки", Page::Instances, app.page),
    ]
    .spacing(6);

    let discover = column![
        section_label("ОТКРЫТЬ"),
        nav_button("◇", "Моды", Page::Catalog, app.page),
        nav_button("▤", "Модпаки", Page::Modpacks, app.page),
    ]
    .spacing(6);

    let system = column![
        section_label("СИСТЕМА"),
        nav_button("⇩", "Загрузки", Page::Downloads, app.page),
        nav_button("♙", "Аккаунты", Page::Accounts, app.page),
        nav_button("⚙", "Настройки", Page::Settings, app.page),
    ]
    .spacing(6);

    let platforms = column![
        section_label("ПЛАТФОРМЫ"),
        platform_nav("▣", "Minecraft Java", PlatformChoice::Java, app.platform),
        platform_nav("◈", "Minecraft Horizon", PlatformChoice::Horizon, app.platform),
    ]
    .spacing(6);

    let profile = container(
        row![
            container(text("U").size(14).color(CYAN))
                .width(38)
                .height(38)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE)),
            column![
                text("Локальный профиль").size(10).color(TEXT),
                row![
                    text("●").size(8).color(GREEN),
                    text("Не авторизован").size(9).color(MUTED),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
            text("⌄").size(16).color(MUTED),
        ]
        .align_y(Alignment::Center)
        .spacing(9),
    )
    .padding(10)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL_LIGHT, EDGE));

    container(
        column![
            brand,
            Space::new().height(23),
            library,
            Space::new().height(14),
            discover,
            Space::new().height(14),
            system,
            Space::new().height(14),
            rule::horizontal(1).style(|_| rule::Style {
                color: DIVIDER,
                radius: iced::border::Radius::default(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }),
            Space::new().height(5),
            platforms,
            Space::new().height(Length::Fill),
            profile,
        ]
        .spacing(0)
        .height(Length::Fill),
    )
    .width(Length::Fixed(238.0))
    .height(Length::Fill)
    .padding(12)
    .style(|_| glass_style(SIDEBAR, EDGE))
    .into()
}

fn section_label(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(9).color(SUBTLE))
        .padding([5, 10])
        .into()
}

fn nav_button(
    icon: &'static str,
    label: &'static str,
    page: Page,
    current: Page,
) -> Element<'static, Message> {
    let active = page == current;
    let background = if active {
        Color::from_rgba(0.19, 0.42, 0.62, 0.58)
    } else {
        Color::TRANSPARENT
    };
    let edge = if active { EDGE_BRIGHT } else { Color::TRANSPARENT };
    let foreground = if active { TEXT } else { MUTED };
    let icon_color = if active { CYAN } else { SUBTLE };

    button(
        row![
            text(icon).size(17).color(icon_color),
            text(label).size(11).color(foreground),
        ]
        .spacing(13)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Navigate(page))
    .width(Length::Fill)
    .padding([10, 12])
    .style(move |theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(background.into());
        style.text_color = foreground;
        style.border = Border::default().rounded(11).width(1).color(edge);
        style
    })
    .into()
}

fn platform_nav(
    icon: &'static str,
    label: &'static str,
    platform: PlatformChoice,
    current: PlatformChoice,
) -> Element<'static, Message> {
    let active = platform == current;
    let foreground = if active { TEXT } else { MUTED };
    let accent = if platform == PlatformChoice::Java { GREEN } else { ORANGE };

    button(
        row![
            text(icon).size(17).color(accent),
            text(label).size(10).color(foreground),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SelectPlatform(platform))
    .width(Length::Fill)
    .padding([8, 10])
    .style(move |theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(
            if active {
                Color::from_rgba(0.15, 0.24, 0.33, 0.48)
            } else {
                Color::TRANSPARENT
            }
            .into(),
        );
        style.text_color = foreground;
        style.border = Border::default().rounded(9);
        style
    })
    .into()
}

fn top_bar(app: &App) -> Element<'_, Message> {
    let search = container(
        row![
            text("⌕").size(21).color(MUTED),
            text_input("Поиск по сборкам, модам и модпакам...", &app.search)
                .on_input(Message::SearchChanged)
                .size(12)
                .padding([10, 8])
                .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([2, 14])
    .width(Length::Fill)
    .style(|_| glass_style(Color::from_rgba(0.18, 0.22, 0.34, 0.68), EDGE));

    row![
        search,
        round_action("◉", Message::Info("Discord-сообщество можно будет подключить в настройках.")),
        round_action("◎", Message::Info("Веб-каталог откроется после подключения источников.")),
        round_action("☼", Message::Info("Переключение темы появится в настройках.")),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([3, 2])
    .into()
}

fn round_action(
    icon: &'static str,
    message: Message,
) -> Element<'static, Message> {
    button(text(icon).size(16).color(TEXT))
        .on_press(message)
        .width(40)
        .height(40)
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(PANEL_LIGHT.into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(20).width(1).color(EDGE);
            style
        })
        .into()
}

fn main_content(app: &App) -> Element<'_, Message> {
    if app.page == Page::Home {
        home_page(app)
    } else {
        secondary_page(app)
    }
}

fn home_page(app: &App) -> Element<'_, Message> {
    let hero = hero_banner(app);
    let platforms = row![
        platform_card(
            app.art.clone(),
            PlatformChoice::Java,
            app.platform == PlatformChoice::Java,
        ),
        platform_card(
            app.art.clone(),
            PlatformChoice::Horizon,
            app.platform == PlatformChoice::Horizon,
        ),
    ]
    .spacing(14)
    .width(Length::Fill);

    let builds = column![
        row![
            column![
                text("Популярные сборки").size(17).color(TEXT),
                text("Подборки для твоей следующей игры").size(9).color(MUTED),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            button(text("Все сборки  →").size(10).color(CYAN))
                .on_press(Message::Navigate(Page::Modpacks))
                .style(button::text),
        ]
        .align_y(Alignment::Center),
        row![
            pack_card(app.art.clone(), "TechnoMagic 2", "Магия и технологии", 0),
            pack_card(app.art.clone(), "All The Mods 9", "Большая универсальная сборка", 1),
            pack_card(app.art.clone(), "Better Minecraft", "Приключения и исследование", 2),
            pack_card(app.art.clone(), "RLCraft", "Хардкорное выживание", 3),
        ]
        .spacing(11)
        .width(Length::Fill),
    ]
    .spacing(10);

    let mut content = column![hero, platforms, builds]
        .spacing(16)
        .width(Length::Fill);

    if let Some(notice) = &app.notice {
        content = content.push(
            container(text(notice).size(10).color(TEXT))
                .padding([10, 14])
                .width(Length::Fill)
                .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT)),
        );
    }

    content.into()
}

fn hero_banner(app: &App) -> Element<'_, Message> {
    let overlay = container(
        row![
            column![
                text("GlowstoneCL").size(36).color(TEXT),
                text("Твой лаунчер для Minecraft и Horizon").size(16).color(TEXT),
                Space::new().height(16),
                row![
                    primary_button("▶   Играть", Message::Info("Сначала создай и установи игровой профиль.")),
                    round_action("⌄", Message::Navigate(Page::Instances)),
                    Space::new().width(Length::Fill),
                    tonal_button("⚙  Настройки сборки", Message::Navigate(Page::Instances)),
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .padding(25),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
        background: Some(Color::from_rgba(0.025, 0.045, 0.085, 0.37).into()),
        border: Border::default().rounded(18).width(1).color(EDGE),
        text_color: Some(TEXT),
        ..Default::default()
    });

    container(
        stack![
            image::Image::new(app.art.clone())
                .width(Length::Fill)
                .height(Length::Fixed(244.0))
                .content_fit(ContentFit::Cover)
                .border_radius(18.0),
            overlay,
        ]
        .width(Length::Fill)
        .height(Length::Fixed(244.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(244.0))
    .style(|_| glass_style(PANEL, EDGE_BRIGHT))
    .into()
}

fn platform_card(
    art: image::Handle,
    platform: PlatformChoice,
    selected: bool,
) -> Element<'static, Message> {
    let (title, subtitle, description, tint, accent, tags, crop) = match platform {
        PlatformChoice::Java => (
            "Minecraft Java",
            "Классический Minecraft",
            "с модами и модпаками",
            Color::from_rgba(0.035, 0.19, 0.09, 0.64),
            GREEN,
            ["Forge", "Fabric", "NeoForge", "Quilt"],
            Rectangle { x: 0, y: 115, width: 700, height: 255 },
        ),
        PlatformChoice::Horizon => (
            "Minecraft Horizon",
            "Выживание в мире Rust",
            "с нативными модами",
            Color::from_rgba(0.23, 0.10, 0.045, 0.66),
            ORANGE,
            ["Моды", "Сборки", "HMOD", "HNMOD"],
            Rectangle { x: 190, y: 80, width: 510, height: 270 },
        ),
    };

    let chips = row![
        tag_chip(tags[0]),
        tag_chip(tags[1]),
        tag_chip(tags[2]),
        tag_chip(tags[3]),
    ]
    .spacing(6);

    let foreground = container(
        row![
            column![
                Space::new().height(4),
                text(title).size(15).color(TEXT),
                text(subtitle).size(10).color(TEXT),
                text(description).size(10).color(MUTED),
                Space::new().height(7),
                chips,
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            button(text("→").size(20).color(TEXT))
                .on_press(Message::SelectPlatform(platform))
                .width(40)
                .height(40)
                .style(|theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(Color::from_rgba(0.12, 0.17, 0.24, 0.82).into());
                    style.text_color = TEXT;
                    style.border = Border::default().rounded(20).color(EDGE);
                    style
                }),
        ]
        .align_y(Alignment::Center)
        .spacing(12)
        .padding(17),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
        background: Some(tint.into()),
        border: Border::default()
            .rounded(16)
            .width(if selected { 1.5 } else { 1.0 })
            .color(if selected { accent } else { EDGE }),
        text_color: Some(TEXT),
        ..Default::default()
    });

    container(
        stack![
            image::Image::new(art)
                .crop(crop)
                .width(Length::Fill)
                .height(Length::Fixed(166.0))
                .content_fit(ContentFit::Cover)
                .border_radius(16.0),
            foreground,
        ]
        .width(Length::Fill)
        .height(Length::Fixed(166.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(166.0))
    .style(move |_| glass_style(
        if platform == PlatformChoice::Java {
            Color::from_rgba(0.04, 0.15, 0.07, 0.55)
        } else {
            Color::from_rgba(0.18, 0.09, 0.06, 0.55)
        },
        if selected { accent } else { EDGE },
    ))
    .into()
}

fn tag_chip(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(8).color(TEXT))
        .padding([5, 8])
        .style(|_| glass_style(Color::from_rgba(0.06, 0.09, 0.14, 0.72), EDGE))
        .into()
}

fn pack_card(
    art: image::Handle,
    title: &'static str,
    subtitle: &'static str,
    index: usize,
) -> Element<'static, Message> {
    let crops = [
        Rectangle { x: 0, y: 20, width: 300, height: 220 },
        Rectangle { x: 220, y: 0, width: 320, height: 240 },
        Rectangle { x: 360, y: 60, width: 330, height: 220 },
        Rectangle { x: 80, y: 160, width: 400, height: 210 },
    ];
    let crop = crops[index.min(3)];

    container(
        stack![
            image::Image::new(art)
                .crop(crop)
                .width(Length::Fill)
                .height(Length::Fixed(142.0))
                .content_fit(ContentFit::Cover)
                .border_radius(13.0),
            container(
                column![
                    Space::new().height(Length::Fill),
                    text(title).size(10).color(TEXT),
                    text(subtitle).size(8).color(MUTED),
                ]
                .spacing(3)
                .padding(10),
            )
            .width(Length::Fill)
            .height(Length::Fixed(142.0))
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.015, 0.025, 0.045, 0.49).into()),
                border: Border::default().rounded(13).color(EDGE),
                text_color: Some(TEXT),
                ..Default::default()
            }),
        ]
        .width(Length::Fill)
        .height(Length::Fixed(142.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(142.0))
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn right_rail(_app: &App) -> Element<'_, Message> {
    let news = container(
        column![
            row![
                text("Новости").size(14).color(TEXT),
                Space::new().width(Length::Fill),
                button(text("Все новости  →").size(9).color(CYAN))
                    .on_press(Message::Info("Новостные ленты появятся после подключения источников."))
                    .style(button::text),
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1).style(|_| rule::Style {
                color: DIVIDER,
                radius: iced::border::Radius::default(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }),
            news_item("M", "Minecraft Java", "Новости и релизы игры", CYAN),
            news_item("F", "Forge / NeoForge", "Обновления загрузчиков", ORANGE),
            news_item("H", "Horizon", "Новости Rust-версии", PURPLE),
        ]
        .spacing(13),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let quick = container(
        column![
            text("Быстрый запуск").size(14).color(TEXT),
            quick_item("▣", "Последняя сборка", "Пока не создана", Page::Instances),
            quick_item("◈", "Последний профиль", "Не выбран", Page::Accounts),
        ]
        .spacing(13),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let status = container(
        row![
            container(text("●").size(13).color(GREEN))
                .width(36)
                .height(36)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(Color::from_rgba(0.04, 0.24, 0.12, 0.72), EDGE)),
            column![
                text("Интерфейс готов").size(10).color(TEXT),
                text("GlowstoneCL v0.1.0").size(8).color(MUTED),
            ]
            .spacing(3),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding(14)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    column![news, quick, status]
        .spacing(13)
        .width(Length::Fill)
        .into()
}

fn news_item(
    mark: &'static str,
    title: &'static str,
    description: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    row![
        container(text(mark).size(15).color(accent))
            .width(38)
            .height(38)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| glass_style(
                Color::from_rgba(accent.r, accent.g, accent.b, 0.14),
                EDGE,
            )),
        column![
            text(title).size(10).color(TEXT),
            text(description).size(9).color(MUTED),
            text("Источник будет подключён").size(8).color(SUBTLE),
        ]
        .spacing(4),
    ]
    .spacing(10)
    .align_y(Alignment::Start)
    .into()
}

fn quick_item(
    mark: &'static str,
    title: &'static str,
    subtitle: &'static str,
    page: Page,
) -> Element<'static, Message> {
    button(
        row![
            container(text(mark).size(14).color(CYAN))
                .width(34)
                .height(34)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE)),
            column![
                text(title).size(9).color(MUTED),
                text(subtitle).size(10).color(TEXT),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
            text("▶").size(11).color(CYAN),
        ]
        .align_y(Alignment::Center)
        .spacing(9),
    )
    .on_press(Message::Navigate(page))
    .width(Length::Fill)
    .padding(4)
    .style(|theme, status| {
        let mut style = button::text(theme, status);
        style.background = Some(Color::TRANSPARENT.into());
        style.text_color = TEXT;
        style
    })
    .into()
}

fn secondary_page(app: &App) -> Element<'_, Message> {
    let (title, subtitle, empty, action) = match app.page {
        Page::Instances => (
            "Мои сборки",
            "Управляй экземплярами Minecraft Java и Horizon.",
            "Здесь появятся твои игровые профили.",
            "Мастер создания сборки будет добавлен следующим этапом.",
        ),
        Page::Catalog => (
            "Каталог модов",
            "Поиск по Modrinth, CurseForge и каталогу Horizon.",
            "Каталоги пока не подключены.",
            "Скоро здесь появятся поиск, фильтры и установка модов.",
        ),
        Page::Modpacks => (
            "Модпаки",
            "Импортируй и создавай собственные игровые сборки.",
            "Модпаков пока нет.",
            "Планируется импорт .mrpack и CurseForge ZIP.",
        ),
        Page::Downloads => (
            "Загрузки",
            "Очередь файлов, прогресс и проверка целостности.",
            "Нет активных загрузок.",
            "Загрузки появятся здесь после подключения установщика.",
        ),
        Page::Accounts => (
            "Аккаунты",
            "Microsoft и локальные игровые профили.",
            "Аккаунты ещё не добавлены.",
            "Авторизация Microsoft и offline-профили будут добавлены отдельно.",
        ),
        Page::Settings => (
            "Настройки",
            "Параметры GlowstoneCL и игровых сред.",
            "Настройки лаунчера",
            "Нативные Wayland и X11 backend включены в Linux-сборку.",
        ),
        Page::Home => ("Главная", "", "", ""),
    };

    let content = container(
        column![
            text(title).size(24).color(TEXT),
            text(subtitle).size(11).color(MUTED),
            Space::new().height(12),
            container(
                column![
                    text(empty).size(16).color(TEXT),
                    text(action).size(10).color(MUTED),
                    primary_button("←  На главную", Message::Navigate(Page::Home)),
                ]
                .spacing(12),
            )
            .padding(22)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE)),
        ]
        .spacing(10),
    )
    .padding(18)
    .width(Length::Fill)
    .into();

    content
}

fn primary_button(
    label: &'static str,
    message: Message,
) -> Element<'static, Message> {
    button(text(label).size(10))
        .on_press(message)
        .padding([10, 15])
        .style(|theme, status| {
            let mut style = button::primary(theme, status);
            style.background = Some(CYAN.into());
            style.text_color = Color::from_rgb8(8, 24, 34);
            style.border = Border::default().rounded(11).width(1).color(CYAN);
            style.shadow = Shadow {
                color: Color::from_rgba(0.1, 0.7, 0.95, 0.2),
                offset: Vector::new(0.0, 3.0),
                blur_radius: 12.0,
            };
            style
        })
        .into()
}

fn tonal_button(
    label: &'static str,
    message: Message,
) -> Element<'static, Message> {
    button(text(label).size(9).color(TEXT))
        .on_press(message)
        .padding([10, 14])
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(Color::from_rgba(0.14, 0.20, 0.31, 0.82).into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(20).width(1).color(EDGE);
            style
        })
        .into()
}

fn glass_style(background: Color, edge: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: Border::default().rounded(16).width(1).color(edge),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.24),
            offset: Vector::new(0.0, 7.0),
            blur_radius: 20.0,
        },
        text_color: Some(TEXT),
        ..Default::default()
    }
}
