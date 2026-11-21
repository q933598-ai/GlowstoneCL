use iced::widget::{
    button, canvas, column, container, image, row, rule, scrollable, stack, svg, text,
    text_input, Space,
};
use iced::{
    Alignment, Border, Color, ContentFit, Element, Length, Rectangle, Shadow, Vector,
};
use glowstone_core::{GameInstance, GamePlatform, InstanceState, LoaderKind};

use crate::auth::{self, DeviceCodePrompt, PollResult};
use crate::game::{self, LaunchOutcome};
use crate::horizon_mods;
use crate::mods;
use crate::store::{self, AccountKind, AccountProfile, PersistedState};

const BG: Color = Color::from_rgb8(10, 15, 26);
const SIDEBAR: Color = Color::from_rgba(0.055, 0.085, 0.15, 0.94);
const PANEL: Color = Color::from_rgba(0.09, 0.13, 0.21, 0.88);
const PANEL_LIGHT: Color = Color::from_rgba(0.15, 0.20, 0.31, 0.72);
const TEXT: Color = Color::from_rgb8(242, 246, 252);
const MUTED: Color = Color::from_rgb8(164, 177, 198);
const SUBTLE: Color = Color::from_rgb8(119, 137, 164);
const CYAN: Color = Color::from_rgb8(105, 218, 235);
const CYAN_DARK: Color = Color::from_rgb8(17, 47, 64);
const PURPLE: Color = Color::from_rgb8(164, 133, 255);
const GREEN: Color = Color::from_rgb8(105, 225, 126);
const ORANGE: Color = Color::from_rgb8(255, 157, 94);
const RED: Color = Color::from_rgb8(255, 112, 125);
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
    Wardrobe,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformChoice {
    Java,
    Horizon,
    HorizonOffline,
}

impl From<PlatformChoice> for GamePlatform {
    fn from(value: PlatformChoice) -> Self {
        match value {
            PlatformChoice::Java => Self::Java,
            PlatformChoice::Horizon => Self::Horizon,
            PlatformChoice::HorizonOffline => Self::HorizonOffline,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    SelectPlatform(PlatformChoice),
    SelectLoader(LoaderKind),
    SearchChanged(String),
    InstanceNameChanged(String),
    InstanceVersionChanged(String),
    VersionQueryChanged(String),
    VersionSelected(String),
    ReloadVersionCatalog,
    VersionCatalogLoaded(GamePlatform, Result<Vec<game::VersionChoice>, String>),
    SearchMods,
    ModTargetSelected(String),
    OpenInstanceMods(String),
    ModsSearched(String, Result<Vec<mods::ModProject>, String>),
    InstallMod(String),
    ModInstallFinished(String, Result<Vec<mods::InstalledMod>, String>),
    RemoveMod(String, String),
    HorizonModsSearched(String, Result<Vec<horizon_mods::HorizonModProject>, String>),
    InstallHorizonMod(String),
    HorizonModInstallFinished(String, Result<Vec<horizon_mods::InstalledHorizonMod>, String>),
    RemoveHorizonMod(String, String),
    CreateInstance,
    SelectInstance(String),
    LaunchInstance(String),
    LaunchFinished(String, Result<LaunchOutcome, String>),
    DeleteInstance(String),
    OfflineNameChanged(String),
    AddOfflineAccount,
    SelectAccount(String),
    DeleteAccount(String),
    SelectWardrobeAccount(String),
    SkinNameChanged(String),
    RenameNameChanged(String),
    SkinModelChanged(store::SkinModel),
    PickSkinFile,
    SkinFilePicked {
        account_uuid: String,
        requested_name: String,
        model: store::SkinModel,
        result: Result<Option<(String, Vec<u8>)>, String>,
    },
    SelectSkin(String),
    ActivateSkin(String),
    RenameSelectedSkin,
    SetSkinModel(String, store::SkinModel),
    DeleteSkin(String),
    ApplySkin(String),
    SkinApplied(String, String, Result<(), String>),
    MicrosoftSkinLoaded(String, Result<Option<(Vec<u8>, store::SkinModel)>, String>),
    RotateSkinPreview(f32),
    MicrosoftClientIdChanged(String),
    SaveMicrosoftClientId,
    StartMicrosoftLogin,
    BrowserLoginFinished(Result<auth::BrowserLoginResult, String>),
    DeviceCodeReady(Result<DeviceCodePrompt, String>),
    OpenMicrosoftVerification,
    PollMicrosoftLogin,
    MicrosoftPollFinished(Result<PollResult, String>),
    Info(&'static str),
}

#[derive(Debug)]
pub struct App {
    page: Page,
    platform: PlatformChoice,
    loader: LoaderKind,
    search: String,
    notice: Option<String>,
    art: image::Handle,
    data: PersistedState,
    instance_name: String,
    instance_version: String,
    version_query: String,
    version_choices: Vec<game::VersionChoice>,
    version_platform: Option<GamePlatform>,
    versions_busy: bool,
    versions_error: Option<String>,
    mod_target_instance: Option<String>,
    mod_results: Vec<mods::ModProject>,
    installed_mods: Vec<mods::InstalledMod>,
    horizon_mod_results: Vec<horizon_mods::HorizonModProject>,
    horizon_installed_mods: Vec<horizon_mods::InstalledHorizonMod>,
    mods_busy: bool,
    mods_busy_project: Option<String>,
    mods_error: Option<String>,
    offline_name: String,
    client_id: String,
    device_code: Option<DeviceCodePrompt>,
    auth_busy: bool,
    launch_busy: Option<String>,
    wardrobe_account_id: Option<String>,
    wardrobe_skins: Vec<store::SkinEntry>,
    wardrobe_active_skin_id: Option<String>,
    wardrobe_selected_skin_id: Option<String>,
    skin_name: String,
    rename_skin_name: String,
    skin_model: store::SkinModel,
    wardrobe_preview: Option<std::sync::Arc<Vec<[u8; 4]>>>,
    wardrobe_angle: f32,
}

impl Default for App {
    fn default() -> Self {
        let data = store::load();
        let wardrobe_account_id = data.settings.selected_account_id.as_ref()
            .and_then(|id| data.accounts.iter().find(|account| &account.id == id))
            .map(|account| account.id.clone())
            .or_else(|| data.accounts.first().map(|account| account.id.clone()));
        let wardrobe_collection = wardrobe_account_id.as_ref()
            .and_then(|id| data.accounts.iter().find(|account| &account.id == id))
            .and_then(|account| store::load_skin_collection(&account.uuid).ok())
            .unwrap_or_default();
        let wardrobe_selected_skin_id = wardrobe_collection.active_skin_id.clone()
            .or_else(|| wardrobe_collection.skins.first().map(|skin| skin.id.clone()));
        Self {
            page: Page::Home,
            platform: PlatformChoice::Java,
            loader: LoaderKind::Vanilla,
            search: String::new(),
            notice: None,
            art: image::Handle::from_bytes(&include_bytes!("../assets/hero.jpg")[..]),
            instance_name: String::new(),
            instance_version: "stable".to_owned(),
            version_query: String::new(),
            version_choices: Vec::new(),
            version_platform: None,
            versions_busy: false,
            versions_error: None,
            mod_target_instance: None,
            mod_results: Vec::new(),
            installed_mods: Vec::new(),
            horizon_mod_results: Vec::new(),
            horizon_installed_mods: Vec::new(),
            mods_busy: false,
            mods_busy_project: None,
            mods_error: None,
            offline_name: String::new(),
            client_id: data.settings.microsoft_client_id.clone(),
            device_code: None,
            auth_busy: false,
            launch_busy: None,
            wardrobe_account_id,
            wardrobe_skins: wardrobe_collection.skins,
            wardrobe_active_skin_id: wardrobe_collection.active_skin_id,
            wardrobe_selected_skin_id,
            skin_name: String::new(),
            rename_skin_name: String::new(),
            skin_model: store::SkinModel::Classic,
            wardrobe_preview: None,
            wardrobe_angle: 0.0,
            data,
        }
    }
}

pub fn update(app: &mut App, message: Message) -> iced::Task<Message> {
    match message {
        Message::Navigate(page) => {
            app.page = page;
            app.notice = None;
            if page == Page::Wardrobe {
                ensure_wardrobe_account(app);
                refresh_wardrobe(app);
                if let Some(account) = wardrobe_account(app)
                    .filter(|account| account.kind == AccountKind::Microsoft)
                    .cloned()
                {
                    let uuid = account.uuid.clone();
                    return iced::Task::perform(
                        game::fetch_microsoft_skin(account, app.client_id.clone()),
                        move |result| Message::MicrosoftSkinLoaded(uuid, result),
                    );
                }
            }
            if page == Page::Instances && app.version_platform != Some(app.platform.into()) {
                let platform = app.platform.into();
                app.versions_busy = true;
                app.versions_error = None;
                return iced::Task::perform(
                    game::available_versions(platform),
                    move |result| Message::VersionCatalogLoaded(platform, result),
                );
            }
            if page == Page::Catalog {
                let selected = app.data.settings.selected_instance_id.as_deref()
                    .and_then(|id| app.data.instances.iter().find(|item| item.id.0 == id))
                    .filter(|item| item.platform == GamePlatform::Java && !matches!(item.loader, LoaderKind::Vanilla))
                    .cloned()
                    .or_else(|| app.data.instances.iter()
                        .find(|item| item.platform == GamePlatform::Java && !matches!(item.loader, LoaderKind::Vanilla))
                        .cloned());
                app.mod_target_instance = selected.as_ref().map(|item| item.id.0.clone());
                app.mods_busy_project = None;
                app.mod_results.clear();
                app.mods_error = None;
                app.installed_mods = selected.as_ref()
                    .and_then(|item| mods::installed(&item.id.0).ok())
                    .unwrap_or_default();
                if let Some(instance) = selected {
                    let target_id = instance.id.0.clone();
                    app.mods_busy = true;
                    return iced::Task::perform(
                        mods::search(instance, app.search.clone()),
                        move |result| Message::ModsSearched(target_id, result),
                    );
                }
                app.mods_error = Some("Сначала создай сборку Minecraft Java с Fabric, Forge, NeoForge или Quilt.".to_owned());
            }
        }
        Message::SelectPlatform(platform) => {
            app.platform = platform;
            app.loader = match platform {
                PlatformChoice::Java => LoaderKind::Vanilla,
                PlatformChoice::Horizon | PlatformChoice::HorizonOffline => LoaderKind::HorizonContent,
            };
            app.instance_version = "stable".to_owned();
            app.version_query.clear();
            app.version_choices.clear();
            app.version_platform = None;
            app.versions_busy = true;
            app.versions_error = None;
            app.notice = None;
            let game_platform = platform.into();
            return iced::Task::perform(
                game::available_versions(game_platform),
                move |result| Message::VersionCatalogLoaded(game_platform, result),
            );
        }
        Message::SelectLoader(loader) => {
            if loader.is_valid_for(app.platform.into()) {
                app.loader = loader;
            }
        }
        Message::SearchChanged(value) => {
            app.search = value;
            app.page = Page::Catalog;
            app.notice = None;
            if app.mod_target_instance.is_none() {
                let selected = app.data.settings.selected_instance_id.as_deref()
                    .and_then(|id| app.data.instances.iter().find(|item| item.id.0 == id))
                    .filter(|item| item.platform == GamePlatform::Java && !matches!(item.loader, LoaderKind::Vanilla))
                    .or_else(|| app.data.instances.iter()
                        .find(|item| item.platform == GamePlatform::Java && !matches!(item.loader, LoaderKind::Vanilla)));
                app.mod_target_instance = selected.map(|item| item.id.0.clone());
                app.installed_mods = app.mod_target_instance.as_deref()
                    .and_then(|id| mods::installed(id).ok())
                    .unwrap_or_default();
            }
        }
        Message::SearchMods => {
            let Some(target_id) = app.mod_target_instance.clone() else {
                app.mods_error = Some("Сначала выбери сборку Minecraft Java с мод-загрузчиком.".to_owned());
                return iced::Task::none();
            };
            let Some(instance) = app.data.instances.iter().find(|item| item.id.0 == target_id).cloned() else {
                app.mods_error = Some("Выбранная сборка больше не существует.".to_owned());
                return iced::Task::none();
            };
            app.mods_busy = true;
            app.mods_error = None;
            return iced::Task::perform(
                mods::search(instance, app.search.clone()),
                move |result| Message::ModsSearched(target_id, result),
            );
        }
        Message::ModTargetSelected(target_id) => {
            let Some(instance) = app.data.instances.iter()
                .find(|item| item.id.0 == target_id
                    && item.platform == GamePlatform::Java
                    && !matches!(item.loader, LoaderKind::Vanilla))
                .cloned()
            else {
                app.mods_error = Some("Нужна Java-сборка с Fabric, Forge, NeoForge или Quilt.".to_owned());
                return iced::Task::none();
            };
            app.mod_target_instance = Some(target_id.clone());
            app.mods_busy_project = None;
            app.installed_mods = mods::installed(&target_id).unwrap_or_default();
            app.mod_results.clear();
            app.mods_busy = true;
            app.mods_error = None;
            return iced::Task::perform(
                mods::search(instance, app.search.clone()),
                move |result| Message::ModsSearched(target_id, result),
            );
        }
        Message::ModsSearched(target_id, result) => {
            if app.mod_target_instance.as_deref() == Some(target_id.as_str()) {
                app.mods_busy = false;
                match result {
                    Ok(results) => {
                        app.mod_results = results;
                        app.mods_error = None;
                    }
                    Err(error) => {
                        app.mod_results.clear();
                        app.mods_error = Some(error);
                    }
                }
            }
        }
        Message::InstallMod(project_id) => {
            if app.mods_busy {
                app.notice = Some("Дождись завершения текущего поиска или установки.".to_owned());
                return iced::Task::none();
            }
            let Some(target_id) = app.mod_target_instance.clone() else {
                app.notice = Some("Сначала выбери Java-сборку.".to_owned());
                return iced::Task::none();
            };
            let Some(instance) = app.data.instances.iter().find(|item| item.id.0 == target_id).cloned() else {
                app.notice = Some("Выбранная сборка не найдена.".to_owned());
                return iced::Task::none();
            };
            app.mods_busy = true;
            app.mods_busy_project = Some(project_id.clone());
            app.notice = Some("Скачиваю мод и проверяю контрольную сумму…".to_owned());
            return iced::Task::perform(
                mods::install(instance, project_id),
                move |result| Message::ModInstallFinished(target_id, result),
            );
        }
        Message::ModInstallFinished(target_id, result) => {
            if app.mod_target_instance.as_deref() == Some(target_id.as_str()) {
                app.mods_busy = false;
                app.mods_busy_project = None;
                match result {
                    Ok(installed) => {
                        let added = installed.iter().filter(|item| item.dependency).count();
                        app.installed_mods = installed;
                        app.notice = Some(if added > 0 {
                            format!("Мод установлен. Также добавлено обязательных зависимостей: {added}.")
                        } else {
                            "Мод успешно установлен в выбранную сборку.".to_owned()
                        });
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
        }
        Message::RemoveMod(target_id, project_id) => {
            if app.mod_target_instance.as_deref() == Some(target_id.as_str()) {
                match mods::remove(&target_id, &project_id) {
                    Ok(installed) => {
                        app.installed_mods = installed;
                        app.notice = Some("Мод удалён из этой сборки.".to_owned());
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
        }
        Message::InstanceNameChanged(value) => app.instance_name = value,
        Message::InstanceVersionChanged(value) => app.instance_version = value,
        Message::VersionQueryChanged(value) => app.version_query = value,
        Message::VersionSelected(value) => {
            app.instance_version = value;
            app.notice = None;
        }
        Message::ReloadVersionCatalog => {
            let platform = app.platform.into();
            app.versions_busy = true;
            app.versions_error = None;
            return iced::Task::perform(
                game::available_versions(platform),
                move |result| Message::VersionCatalogLoaded(platform, result),
            );
        }
        Message::VersionCatalogLoaded(platform, result) => {
            if platform == app.platform.into() {
                app.versions_busy = false;
                app.version_platform = Some(platform);
                match result {
                    Ok(choices) => {
                        app.version_choices = choices;
                        app.versions_error = None;
                    }
                    Err(error) => {
                        app.version_choices.clear();
                        app.versions_error = Some(error);
                    }
                }
            }
        }
        Message::CreateInstance => {
            let name = app.instance_name.trim().to_owned();
            let version = app.instance_version.trim().to_owned();

            if name.is_empty() || version.is_empty() {
                app.notice = Some("Укажи название сборки и версию Minecraft.".into());
            } else {
                match store::create_instance(name, version, app.platform.into(), app.loader) {
                    Ok(instance) => {
                        app.data.settings.selected_instance_id = Some(instance.id.0.clone());
                        app.data.instances.push(instance);
                        match store::save(&app.data) {
                            Ok(()) => {
                                app.instance_name.clear();
                                app.notice = Some("Сборка создана и сохранена.".into());
                            }
                            Err(error) => {
                                app.notice = Some(format!("Сборка создана в памяти, но не сохранена: {error}"));
                            }
                        }
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
            app.page = Page::Instances;
        }
        Message::SelectInstance(id) => {
            if app.data.instances.iter().any(|instance| instance.id.0 == id) {
                app.data.settings.selected_instance_id = Some(id);
                app.notice = store::save(&app.data)
                    .err()
                    .map(|error| format!("Не удалось сохранить выбранную сборку: {error}"))
                    .or_else(|| Some("Сборка выбрана.".into()));
            }
        }
        Message::LaunchInstance(id) => {
            if app.launch_busy.is_some() {
                app.notice = Some("Другая сборка уже запускается. Дождись завершения операции.".into());
                return iced::Task::none();
            }
            let Some(instance) = app.data.instances.iter().find(|item| item.id.0 == id).cloned() else {
                app.notice = Some("Сборка не найдена.".into());
                return iced::Task::none();
            };
            let Some(selected_id) = app.data.settings.selected_account_id.as_deref() else {
                app.notice = Some("Сначала выбери игровой аккаунт в разделе «Аккаунты».".into());
                return iced::Task::none();
            };
            let Some(account) = app.data.accounts.iter().find(|item| item.id == selected_id).cloned() else {
                app.notice = Some("Выбранный аккаунт не найден. Выбери аккаунт заново.".into());
                return iced::Task::none();
            };

            app.launch_busy = Some(id.clone());
            app.notice = Some(format!(
                "Подготовка {} {}. При первом запуске файлы игры и Java будут загружены автоматически…",
                instance.platform.label(),
                instance.game_version
            ));
            let client_id = app.client_id.clone();
            return iced::Task::perform(
                game::launch_instance(instance, account, client_id),
                move |result| Message::LaunchFinished(id.clone(), result),
            );
        }
        Message::LaunchFinished(id, result) => {
            if app.launch_busy.as_deref() == Some(id.as_str()) {
                app.launch_busy = None;
            }
            match result {
                Ok(outcome) => {
                    if let Some(instance) = app.data.instances.iter_mut().find(|item| item.id.0 == outcome.instance_id) {
                        instance.state = InstanceState::Ready;
                    }
                    let save_error = store::save(&app.data).err();
                    let mut exit_text = match outcome.exit_code {
                        Some(0) => "Игра завершилась штатно.".to_owned(),
                        Some(code) => format!("Игра завершилась с кодом {code}. Файлы сборки установлены."),
                        None => "Игровой процесс завершился. Файлы сборки установлены.".to_owned(),
                    };
                    if let Some(notice) = outcome.notice {
                        exit_text.push(' ');
                        exit_text.push_str(&notice);
                    }
                    app.notice = Some(match save_error {
                        Some(error) => format!("{exit_text} Не удалось сохранить состояние сборки: {error}"),
                        None => exit_text,
                    });
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::DeleteInstance(id) => {
            if app.launch_busy.as_deref() == Some(id.as_str()) {
                app.notice = Some("Нельзя удалить сборку, пока она запущена.".into());
                return iced::Task::none();
            }
            match store::remove_instance(&id) {
                Ok(()) => {
                    app.data.instances.retain(|instance| instance.id.0 != id);
                    if app.data.settings.selected_instance_id.as_deref() == Some(id.as_str()) {
                        app.data.settings.selected_instance_id = None;
                    }
                    app.notice = store::save(&app.data)
                        .err()
                        .map(|error| format!("Не удалось сохранить удаление: {error}"))
                        .or_else(|| Some("Сборка удалена из библиотеки. Её файлы и миры сохранены в локальной корзине.".into()));
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::OfflineNameChanged(value) => app.offline_name = value,
        Message::AddOfflineAccount => {
            match store::create_offline_account(&app.offline_name) {
                Ok(account) => {
                    if app.data.accounts.iter().any(|item| item.id == account.id) {
                        app.data.settings.selected_account_id = Some(account.id.clone());
                        app.notice = store::save(&app.data)
                            .err()
                            .map(|error| format!("Не удалось выбрать профиль: {error}"))
                            .or_else(|| Some("Этот offline-профиль уже добавлен и выбран.".into()));
                    } else {
                        app.data.settings.selected_account_id = Some(account.id.clone());
                        app.data.accounts.push(account);
                        app.offline_name.clear();
                        app.notice = store::save(&app.data)
                            .err()
                            .map(|error| format!("Профиль добавлен, но не сохранён: {error}"))
                            .or_else(|| Some("Offline-профиль добавлен.".into()));
                    }
                }
                Err(error) => app.notice = Some(error),
            }
            app.page = Page::Accounts;
        }
        Message::SelectAccount(id) => {
            if app.data.accounts.iter().any(|account| account.id == id) {
                app.data.settings.selected_account_id = Some(id.clone());
                app.wardrobe_account_id = Some(id);
                refresh_wardrobe(app);
                app.notice = store::save(&app.data)
                    .err()
                    .map(|error| format!("Не удалось сохранить активный аккаунт: {error}"))
                    .or_else(|| Some("Активный аккаунт изменён.".into()));
            }
        }
        Message::DeleteAccount(id) => {
            if let Some(account) = app.data.accounts.iter().find(|item| item.id == id) {
                if account.kind == AccountKind::Microsoft {
                    store::remove_refresh_token(&account.id);
                }
            }
            app.data.accounts.retain(|account| account.id != id);
            if app.data.settings.selected_account_id.as_deref() == Some(id.as_str()) {
                app.data.settings.selected_account_id = None;
            }
            app.notice = store::save(&app.data)
                .err()
                .map(|error| format!("Не удалось сохранить удаление: {error}"))
                .or_else(|| Some("Аккаунт удалён из GlowstoneCL.".into()));
        }
        Message::SelectWardrobeAccount(account_id) => {
            if app.data.accounts.iter().any(|account| account.id == account_id) {
                app.wardrobe_account_id = Some(account_id);
                app.wardrobe_selected_skin_id = None;
                app.skin_name.clear();
                refresh_wardrobe(app);
                app.notice = None;
                if let Some(account) = wardrobe_account(app)
                    .filter(|account| account.kind == AccountKind::Microsoft)
                    .cloned()
                {
                    let uuid = account.uuid.clone();
                    return iced::Task::perform(
                        game::fetch_microsoft_skin(account, app.client_id.clone()),
                        move |result| Message::MicrosoftSkinLoaded(uuid, result),
                    );
                }
            } else {
                app.notice = Some("Игровой аккаунт не найден.".into());
            }
        }
        Message::SkinNameChanged(value) => app.skin_name = value,
        Message::RenameNameChanged(value) => app.rename_skin_name = value,
        Message::SkinModelChanged(model) => app.skin_model = model,
        Message::PickSkinFile => {
            let Some(account_id) = app.wardrobe_account_id.as_deref() else {
                app.notice = Some("Сначала выбери игровой аккаунт.".into());
                return iced::Task::none();
            };
            let Some(account) = app.data.accounts.iter().find(|account| account.id == account_id) else {
                app.notice = Some("Выбранный игровой аккаунт не найден.".into());
                return iced::Task::none();
            };
            let account_uuid = account.uuid.clone();
            let requested_name = app.skin_name.clone();
            let model = app.skin_model;
            app.notice = Some("Выбери PNG-файл скина размером 64×64 или 64×32.".into());
            return iced::Task::perform(
                async move {
                    let picked = rfd::AsyncFileDialog::new()
                        .add_filter("Minecraft skin (PNG)", &["png"])
                        .pick_file()
                        .await;
                    match picked {
                        Some(file) => {
                            let file_name = file.file_name();
                            let bytes = file.read().await;
                            Ok(Some((file_name, bytes)))
                        }
                        None => Ok(None),
                    }
                },
                move |result| Message::SkinFilePicked {
                    account_uuid,
                    requested_name,
                    model,
                    result,
                },
            );
        }
        Message::SkinFilePicked { account_uuid, requested_name, model, result } => {
            match result {
                Ok(Some((file_name, bytes))) => {
                    let fallback_name = file_name.rsplit_once('.')
                        .map(|(stem, _)| stem)
                        .unwrap_or(file_name.as_str());
                    let name = if requested_name.trim().is_empty() {
                        fallback_name
                    } else {
                        requested_name.trim()
                    };
                    match store::import_skin(&account_uuid, name, &bytes, model) {
                        Ok(collection) => {
                            let last_skin_id = collection.skins.last().map(|skin| skin.id.clone());
                            let active_skin_id = collection.active_skin_id.clone();
                            let is_current = app.wardrobe_account_id.as_ref()
                                .and_then(|id| app.data.accounts.iter().find(|account| &account.id == id))
                                .is_some_and(|account| account.uuid == account_uuid);
                            if is_current {
                                app.wardrobe_skins = collection.skins;
                                app.wardrobe_active_skin_id = active_skin_id;
                                app.wardrobe_selected_skin_id = last_skin_id;
                                refresh_skin_preview(app);
                                app.rename_skin_name = app.wardrobe_selected_skin_id.as_ref()
                                    .and_then(|id| app.wardrobe_skins.iter().find(|skin| &skin.id == id))
                                    .map(|skin| skin.name.clone())
                                    .unwrap_or_default();
                                app.skin_name.clear();
                            }
                            app.notice = Some(format!("Скин «{name}» добавлен в локальный гардероб."));
                        }
                        Err(error) => app.notice = Some(error),
                    }
                }
                Ok(None) => app.notice = None,
                Err(error) => app.notice = Some(error),
            }
        }
        Message::SelectSkin(id) => {
            if let Some(skin) = app.wardrobe_skins.iter().find(|skin| skin.id == id) {
                app.wardrobe_selected_skin_id = Some(id);
                app.rename_skin_name = skin.name.clone();
                refresh_skin_preview(app);
            }
        }
        Message::ActivateSkin(id) => {
            if let Some(account) = wardrobe_account(app) {
                match store::set_active_skin(&account.uuid, &id) {
                    Ok(collection) => {
                        app.wardrobe_skins = collection.skins;
                        app.wardrobe_active_skin_id = collection.active_skin_id;
                        app.wardrobe_selected_skin_id = Some(id.clone());
                        app.rename_skin_name = app.wardrobe_skins.iter()
                            .find(|skin| skin.id == id)
                            .map(|skin| skin.name.clone())
                            .unwrap_or_default();
                        refresh_skin_preview(app);
                        app.notice = Some("Активный скин выбран в локальном гардеробе этого аккаунта.".into());
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
        }
        Message::RenameSelectedSkin => {
            let selected_id = app.wardrobe_selected_skin_id.clone();
            if let (Some(account), Some(skin_id)) = (wardrobe_account(app), selected_id) {
                match store::rename_skin(&account.uuid, &skin_id, &app.rename_skin_name) {
                    Ok(collection) => {
                        app.wardrobe_skins = collection.skins;
                        app.wardrobe_active_skin_id = collection.active_skin_id;
                        app.notice = Some("Название скина обновлено.".into());
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
        }
        Message::SetSkinModel(id, model) => {
            if let Some(account) = wardrobe_account(app) {
                match store::set_skin_model(&account.uuid, &id, model) {
                    Ok(collection) => {
                        app.wardrobe_skins = collection.skins;
                        app.wardrobe_active_skin_id = collection.active_skin_id;
                        app.skin_model = model;
                        app.notice = Some(format!("Модель скина изменена: {}.", model.label()));
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
        }
        Message::DeleteSkin(id) => {
            if let Some(account) = wardrobe_account(app) {
                match store::delete_skin(&account.uuid, &id) {
                    Ok(collection) => {
                        let next_selected = collection.active_skin_id.clone()
                            .or_else(|| collection.skins.first().map(|skin| skin.id.clone()));
                        app.wardrobe_skins = collection.skins;
                        app.wardrobe_active_skin_id = collection.active_skin_id;
                        if app.wardrobe_selected_skin_id.as_deref() == Some(id.as_str()) {
                            app.wardrobe_selected_skin_id = next_selected;
                            app.rename_skin_name.clear();
                        }
                        refresh_skin_preview(app);
                        app.notice = Some("Скин удалён из локального гардероба.".into());
                    }
                    Err(error) => app.notice = Some(error),
                }
            }
        }
        Message::MicrosoftSkinLoaded(account_uuid, result) => {
            let is_current = wardrobe_account(app)
                .is_some_and(|account| account.uuid == account_uuid);
            if is_current {
                match result {
                    Ok(Some((bytes, model))) => {
                        match store::import_skin_if_missing(
                            &account_uuid,
                            "Текущий скин Microsoft",
                            &bytes,
                            model,
                        ) {
                            Ok(collection) => {
                                app.wardrobe_skins = collection.skins;
                                app.wardrobe_active_skin_id = collection.active_skin_id.clone();
                                app.wardrobe_selected_skin_id = collection.active_skin_id
                                    .or_else(|| app.wardrobe_skins.first().map(|skin| skin.id.clone()));
                                app.rename_skin_name = app.wardrobe_selected_skin_id.as_ref()
                                    .and_then(|id| app.wardrobe_skins.iter().find(|skin| &skin.id == id))
                                    .map(|skin| skin.name.clone())
                                    .unwrap_or_default();
                                refresh_skin_preview(app);
                                app.notice = Some("Текущий скин Microsoft-профиля синхронизирован с гардеробом.".into());
                            }
                            Err(error) => app.notice = Some(error),
                        }
                    }
                    Ok(None) => {
                        app.notice = Some("В Minecraft-профиле пока нет установленного скина.".into());
                    }
                    Err(error) => {
                        app.notice = Some(format!("Не удалось получить скин Microsoft-профиля: {error}"));
                    }
                }
            }
        }
        Message::ApplySkin(id) => {
            let Some(account) = wardrobe_account(app).cloned() else {
                app.notice = Some("Сначала выбери игровой аккаунт.".into());
                return iced::Task::none();
            };
            let Some(skin) = app.wardrobe_skins.iter().find(|skin| skin.id == id).cloned() else {
                app.notice = Some("Скин не найден.".into());
                return iced::Task::none();
            };
            if account.kind == AccountKind::Offline {
                match store::set_active_skin(&account.uuid, &id) {
                    Ok(collection) => {
                        app.wardrobe_skins = collection.skins;
                        app.wardrobe_active_skin_id = collection.active_skin_id;
                        app.wardrobe_selected_skin_id = Some(id.clone());
                        app.rename_skin_name = skin.name.clone();
                        refresh_skin_preview(app);
                        app.notice = Some(format!(
                            "Скин «{}» выбран для offline-игры. Он будет применён в Horizon Offline и в совместимых версиях Minecraft Java через Fabric при следующем запуске.",
                            skin.name
                        ));
                    }
                    Err(error) => app.notice = Some(error),
                }
                return iced::Task::none();
            }

            let path = match store::skin_image_path(&account.uuid, &id) {
                Ok(path) => path,
                Err(error) => {
                    app.notice = Some(error);
                    return iced::Task::none();
                }
            };
            let account_uuid = account.uuid.clone();
            app.notice = Some(format!("Загружаю «{}» в профиль Minecraft…", skin.name));
            return iced::Task::perform(
                game::apply_microsoft_skin(account, app.client_id.clone(), path, skin.model),
                move |result| Message::SkinApplied(id.clone(), account_uuid, result),
            );
        }
        Message::SkinApplied(id, account_uuid, result) => {
            match result {
                Ok(()) => {
                    match store::set_active_skin(&account_uuid, &id) {
                        Ok(collection) => {
                            let is_current = wardrobe_account(app)
                                .is_some_and(|account| account.uuid == account_uuid);
                            if is_current {
                                app.wardrobe_skins = collection.skins;
                                app.wardrobe_active_skin_id = collection.active_skin_id;
                                app.wardrobe_selected_skin_id = Some(id);
                                refresh_skin_preview(app);
                            }
                            app.notice = Some("Скин загружен в официальный Minecraft-профиль. При следующем обновлении профиля он будет виден в игре и на других устройствах.".into());
                        }
                        Err(error) => app.notice = Some(format!("Скин отправлен в Minecraft, но не удалось обновить локальный выбор: {error}")),
                    }
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::RotateSkinPreview(delta) => {
            app.wardrobe_angle = (app.wardrobe_angle + delta).rem_euclid(std::f32::consts::TAU);
        }
        Message::MicrosoftClientIdChanged(value) => app.client_id = value,
        Message::SaveMicrosoftClientId => {
            app.data.settings.microsoft_client_id = app.client_id.trim().to_owned();
            app.notice = store::save(&app.data)
                .err()
                .map(|error| format!("Не удалось сохранить Client ID: {error}"))
                .or_else(|| Some("Microsoft Client ID сохранён.".into()));
        }
        Message::StartMicrosoftLogin => {
            app.auth_busy = true;
            let client_id = (!app.client_id.trim().is_empty())
                .then(|| app.client_id.trim().to_owned());

            // Keep the persisted setting in sync with the field. In particular,
            // clearing a previously configured custom ID must switch back to the
            // official Minecraft Launcher public client on this and later launches.
            let persisted_client_id = client_id.clone().unwrap_or_default();
            if app.data.settings.microsoft_client_id != persisted_client_id {
                app.data.settings.microsoft_client_id = persisted_client_id;
                if let Err(error) = store::save(&app.data) {
                    app.auth_busy = false;
                    app.notice = Some(format!("Не удалось сохранить настройки Microsoft: {error}"));
                    return iced::Task::none();
                }
            }

            app.notice = Some("Открываю официальный вход Microsoft в браузере…".into());
            return iced::Task::perform(
                auth::browser_login(client_id),
                Message::BrowserLoginFinished,
            );
        }
        Message::BrowserLoginFinished(result) => {
            app.auth_busy = false;
            match result {
                Ok(login) => {
                    let account_id = login.account.id.clone();
                    app.data.accounts.retain(|current| current.id != account_id);
                    app.data.accounts.push(login.account.clone());
                    app.data.settings.selected_account_id = Some(account_id.clone());
                    app.wardrobe_account_id = Some(account_id.clone());
                    refresh_wardrobe(app);
                    app.device_code = None;

                    let token_status = match store::store_refresh_token(&account_id, &login.refresh_token) {
                        Ok(()) => "Токен обновления сохранён в системном хранилище.",
                        Err(_) => "Не удалось сохранить refresh token в системном хранилище. Для следующего запуска может потребоваться повторный вход.",
                    };
                    let save_status = store::save(&app.data);
                    app.notice = Some(match save_status {
                        Ok(()) => format!("Minecraft-профиль {} подключён. {token_status}", login.account.name),
                        Err(error) => format!("Профиль {} подключён, но список аккаунтов не удалось сохранить: {error}", login.account.name),
                    });
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::DeviceCodeReady(result) => {
            match result {
                Ok(prompt) => {
                    // Mirror Prism Launcher's Device Code fallback: open the Microsoft
                    // verification page and poll in the background without another click.
                    let open_result = webbrowser::open(&prompt.verification_uri);
                    let client_id = app.client_id.trim().to_owned();
                    let device_code = prompt.device_code.clone();
                    let interval = prompt.interval;
                    let expires_in = prompt.expires_in;
                    app.device_code = Some(prompt.clone());
                    app.auth_busy = true;
                    app.notice = Some(match open_result {
                        Ok(()) => "Браузер открыт. Войди в Microsoft — GlowstoneCL сам заметит успешную авторизацию.".to_owned(),
                        Err(error) => format!(
                            "Не удалось автоматически открыть браузер: {error}. Открой {} и введи код {}. Проверка уже запущена.",
                            prompt.verification_uri, prompt.user_code
                        ),
                    });
                    return iced::Task::perform(
                        auth::poll_device_code(client_id, device_code, interval, expires_in),
                        Message::MicrosoftPollFinished,
                    );
                }
                Err(error) => {
                    app.auth_busy = false;
                    app.notice = Some(error);
                }
            }
        }
        Message::OpenMicrosoftVerification => {
            if let Some(prompt) = &app.device_code {
                if let Err(error) = webbrowser::open(&prompt.verification_uri) {
                    app.notice = Some(format!("Не удалось открыть браузер: {error}. Адрес: {}", prompt.verification_uri));
                }
            }
        }
        Message::PollMicrosoftLogin => {
            let Some(prompt) = app.device_code.clone() else {
                app.notice = Some("Сначала запроси код Microsoft.".into());
                return iced::Task::none();
            };
            app.auth_busy = true;
            app.notice = Some("Проверяю подтверждение входа…".into());
            return iced::Task::perform(
                auth::poll_device_code(
                    app.client_id.trim().to_owned(),
                    prompt.device_code,
                    prompt.interval,
                    prompt.expires_in,
                ),
                Message::MicrosoftPollFinished,
            );
        }
        Message::MicrosoftPollFinished(result) => {
            app.auth_busy = false;
            match result {
                Ok(PollResult::SignedIn { account, refresh_token }) => {
                    let account_id = account.id.clone();
                    app.data.accounts.retain(|current| current.id != account_id);
                    app.data.accounts.push(account.clone());
                    app.data.settings.selected_account_id = Some(account_id.clone());
                    app.wardrobe_account_id = Some(account_id.clone());
                    refresh_wardrobe(app);
                    app.device_code = None;

                    let token_status = match store::store_refresh_token(&account_id, &refresh_token) {
                        Ok(()) => "Токен обновления сохранён в системном хранилище.",
                        Err(_) => "Не удалось сохранить refresh token в системном хранилище. Для следующего запуска может потребоваться повторный вход.",
                    };
                    let save_status = store::save(&app.data);
                    app.notice = Some(match save_status {
                        Ok(()) => format!("Minecraft-профиль {} подключён. {token_status}", account.name),
                        Err(error) => format!("Профиль {} подключён, но список аккаунтов не удалось сохранить: {error}", account.name),
                    });
                }
                Err(error) => app.notice = Some(error),
            }
        }
        Message::Info(message) => app.notice = Some(message.to_owned()),
    }

    iced::Task::none()
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
                    .width(Length::Fixed(278.0)),
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

fn draw_glow(frame: &mut canvas::Frame, center: iced::Point, radius: f32, color: Color) {
    for layer in 0..24 {
        let amount = layer as f32 / 23.0;
        let current_radius = radius * (1.0 - amount * 0.94);
        let alpha = 0.001 + (1.0 - amount).powf(2.0) * 0.008;
        let circle = canvas::Path::circle(center, current_radius);
        frame.fill(&circle, Color::from_rgba(color.r, color.g, color.b, alpha));
    }
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let brand = container(
        row![
            container(icon("diamond", CYAN, 24.0))
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
        nav_button("home", "Главная", Page::Home, app.page),
        nav_button("instances", "Мои сборки", Page::Instances, app.page),
    ]
    .spacing(6);

    let discover = column![
        section_label("ОТКРЫТЬ"),
        nav_button("puzzle", "Каталог модов", Page::Catalog, app.page),
        nav_button("package", "Модпаки", Page::Modpacks, app.page),
    ]
    .spacing(6);

    let system = column![
        section_label("СИСТЕМА"),
        nav_button("download", "Загрузки", Page::Downloads, app.page),
        nav_button("user", "Аккаунты", Page::Accounts, app.page),
        nav_button("shirt", "Гардероб", Page::Wardrobe, app.page),
        nav_button("settings", "Настройки", Page::Settings, app.page),
    ]
    .spacing(6);

    let platforms = column![
        section_label("ПЛАТФОРМЫ"),
        platform_nav("minecraft", "Minecraft Java", PlatformChoice::Java, app.platform),
        platform_nav("horizon", "Minecraft Horizon", PlatformChoice::Horizon, app.platform),
        platform_nav("user", "Horizon Offline", PlatformChoice::HorizonOffline, app.platform),
    ]
    .spacing(6);

    let selected_account = app.data.settings.selected_account_id.as_ref()
        .and_then(|id| app.data.accounts.iter().find(|account| &account.id == id));
    let current_account = selected_account
        .map(|account| account.name.as_str())
        .unwrap_or("Гость");
    let profile_subtitle = if selected_account.is_some() {
        "Активный аккаунт"
    } else if app.data.accounts.is_empty() {
        "Не авторизован"
    } else {
        "Выбери аккаунт"
    };

    let profile = container(
        row![
            container(icon("user", CYAN, 18.0))
                .width(38)
                .height(38)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(CYAN_DARK, EDGE)),
            column![
                text(current_account.to_owned()).size(10).color(TEXT),
                row![
                    icon("check", if app.data.accounts.is_empty() { SUBTLE } else { GREEN }, 10.0),
                    text(profile_subtitle).size(9).color(MUTED),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
            icon("chevron-down", MUTED, 14.0),
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
            Space::new().height(8),
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
    icon_name: &'static str,
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
            icon(icon_name, icon_color, 17.0),
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
    icon_name: &'static str,
    label: &'static str,
    platform: PlatformChoice,
    current: PlatformChoice,
) -> Element<'static, Message> {
    let active = platform == current;
    let foreground = if active { TEXT } else { MUTED };
    let accent = if platform == PlatformChoice::Java { GREEN } else { ORANGE };

    button(
        row![
            icon(icon_name, accent, 17.0),
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
            icon("search", MUTED, 18.0),
            text_input("Поиск по сборкам, модам и модпакам...", &app.search)
                .on_input(Message::SearchChanged)
                .on_submit(Message::SearchMods)
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
        round_action("community", Message::Info("Ссылку на сообщество добавим позже.")),
        round_action("globe", Message::Navigate(Page::Catalog)),
        round_action("settings", Message::Navigate(Page::Settings)),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([3, 2])
    .into()
}

fn round_action(icon_name: &'static str, message: Message) -> Element<'static, Message> {
    button(icon(icon_name, TEXT, 16.0))
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
    match app.page {
        Page::Home => home_page(app),
        Page::Instances => instances_page(app),
        Page::Accounts => accounts_page(app),
        Page::Wardrobe => wardrobe_page(app),
        Page::Settings => settings_page(app),
        Page::Catalog => catalog_page(app),
        Page::Modpacks => modpacks_page(),
        Page::Downloads => downloads_page(),
    }
}

fn home_page(app: &App) -> Element<'_, Message> {
    let hero = hero_banner(app);
    let platforms = row![
        platform_card(app.art.clone(), PlatformChoice::Java, app.platform == PlatformChoice::Java),
        platform_card(app.art.clone(), PlatformChoice::Horizon, app.platform == PlatformChoice::Horizon),
        platform_card(app.art.clone(), PlatformChoice::HorizonOffline, app.platform == PlatformChoice::HorizonOffline),
    ]
    .spacing(14)
    .width(Length::Fill);

    let mut build_cards = row![]
        .spacing(11)
        .width(Length::Fill);
    for (index, instance) in app.data.instances.iter().take(4).enumerate() {
        build_cards = build_cards.push(pack_card(
            app.art.clone(),
            instance.name.clone(),
            format!("{} · {}", instance.game_version, loader_label(instance.loader)),
            index,
        ));
    }

    let builds_content: Element<'_, Message> = if app.data.instances.is_empty() {
        container(
            row![
                icon("instances", CYAN, 25.0),
                column![
                    text("Пока нет игровых сборок").size(12).color(TEXT),
                    text("Создай первый профиль Java или Horizon — он появится здесь.")
                        .size(9)
                        .color(MUTED),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                tonal_button("Создать", Message::Navigate(Page::Instances)),
            ]
            .align_y(Alignment::Center)
            .spacing(12),
        )
        .padding(15)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE))
        .into()
    } else {
        build_cards.into()
    };

    let builds = column![
        row![
            column![
                text("Твои сборки").size(16).color(TEXT),
                text("Недавно созданные игровые профили").size(9).color(MUTED),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            button(text("Все сборки").size(10).color(CYAN))
                .on_press(Message::Navigate(Page::Instances))
                .style(button::text),
        ]
        .align_y(Alignment::Center),
        builds_content,
    ]
    .spacing(10);

    let mut content = column![
        hero,
        platforms,
        builds,
    ]
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
    let play_message = app
        .data
        .settings
        .selected_instance_id
        .as_ref()
        .filter(|id| app.data.instances.iter().any(|instance| &instance.id.0 == *id))
        .cloned()
        .map(Message::LaunchInstance)
        .unwrap_or(Message::Navigate(Page::Instances));

    let overlay = container(
        row![
            column![
                text("GlowstoneCL").size(34).color(TEXT),
                text("Твой Minecraft. Твои правила.").size(16).color(TEXT),
                text("Java Edition и Horizon — в одной библиотеке.").size(11).color(MUTED),
                Space::new().height(12),
                row![
                    primary_button(
                        if app.launch_busy.is_some() { "Запускается..." } else { "Играть" },
                        play_message,
                    ),
                    round_action("chevron-down", Message::Navigate(Page::Instances)),
                    Space::new().width(Length::Fill),
                    tonal_button("Настроить сборки", Message::Navigate(Page::Instances)),
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
                .height(Length::Fixed(265.0))
                .content_fit(ContentFit::Cover)
                .border_radius(18.0),
            overlay,
        ]
        .width(Length::Fill)
        .height(Length::Fixed(265.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(265.0))
    .style(|_| glass_style(PANEL, EDGE_BRIGHT))
    .into()
}

fn platform_card(art: image::Handle, platform: PlatformChoice, selected: bool) -> Element<'static, Message> {
    let (title, subtitle, description, tint, accent, tags, crop) = match platform {
        PlatformChoice::Java => (
            "Minecraft Java",
            "Классическая Java Edition",
            "От Vanilla до крупных модпаков",
            Color::from_rgba(0.035, 0.19, 0.09, 0.64),
            GREEN,
            ["Forge", "Fabric", "NeoForge", "Quilt"],
            Rectangle::<u32> { x: 0, y: 115, width: 700, height: 255 },
        ),
        PlatformChoice::Horizon => (
            "Minecraft Horizon",
            "Официальный Rust runtime",
            "Microsoft-аккаунт и официальные Nightly-сборки",
            Color::from_rgba(0.23, 0.10, 0.045, 0.66),
            ORANGE,
            ["HMOD", "HNMOD", "Official", "Online"],
            Rectangle::<u32> { x: 190, y: 80, width: 510, height: 270 },
        ),
        PlatformChoice::HorizonOffline => (
            "Horizon Offline",
            "Неофициальный offline fork",
            "Без Microsoft-входа, нужны локальные файлы Minecraft 26.3",
            Color::from_rgba(0.18, 0.12, 0.25, 0.72),
            PURPLE,
            ["Offline", "26.3", "HMOD", "HNMOD"],
            Rectangle::<u32> { x: 190, y: 80, width: 510, height: 270 },
        ),
    };

    let mut chips = row![];
    for tag in tags {
        chips = chips.push(tag_chip(tag));
    }
    chips = chips.spacing(6);

    let foreground = container(
        row![
            column![
                text(title).size(15).color(TEXT),
                text(subtitle).size(10).color(TEXT),
                text(description).size(9).color(MUTED),
                Space::new().height(5),
                chips,
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            button(icon("arrow-right", TEXT, 18.0))
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
                .height(Length::Fixed(178.0))
                .content_fit(ContentFit::Cover)
                .border_radius(16.0),
            foreground,
        ]
        .width(Length::Fill)
        .height(Length::Fixed(178.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(178.0))
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
    title: String,
    subtitle: String,
    index: usize,
) -> Element<'static, Message> {
    let crops = [
        Rectangle::<u32> { x: 0, y: 20, width: 300, height: 220 },
        Rectangle::<u32> { x: 220, y: 0, width: 320, height: 240 },
        Rectangle::<u32> { x: 360, y: 60, width: 330, height: 220 },
        Rectangle::<u32> { x: 80, y: 160, width: 400, height: 210 },
    ];
    let crop = crops[index.min(3)];

    container(
        stack![
            image::Image::new(art)
                .crop(crop)
                .width(Length::Fill)
                .height(Length::Fixed(140.0))
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
            .height(Length::Fixed(140.0))
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.015, 0.025, 0.045, 0.55).into()),
                border: Border::default().rounded(13).color(EDGE),
                text_color: Some(TEXT),
                ..Default::default()
            }),
        ]
        .width(Length::Fill)
        .height(Length::Fixed(140.0))
        .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(140.0))
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn right_rail(app: &App) -> Element<'_, Message> {
    let news = container(
        column![
            row![
                text("Новости").size(14).color(TEXT),
                Space::new().width(Length::Fill),
                button(text("Все").size(9).color(CYAN))
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
            news_item("minecraft", "Minecraft Java", "Новости и релизы игры", CYAN),
            news_item("curseforge", "Forge и NeoForge", "Обновления загрузчиков", ORANGE),
            news_item("horizon", "Horizon", "Новости Rust runtime", PURPLE),
        ]
        .spacing(13),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let last_instance = app.data.settings.selected_instance_id.as_ref()
        .and_then(|id| app.data.instances.iter().find(|item| &item.id.0 == id))
        .or_else(|| app.data.instances.last())
        .map(|item| item.name.clone())
        .unwrap_or_else(|| "Пока нет сборок".into());
    let last_account = app.data.settings.selected_account_id.as_ref()
        .and_then(|id| app.data.accounts.iter().find(|item| &item.id == id))
        .map(|item| item.name.clone())
        .unwrap_or_else(|| "Не авторизован".into());

    let quick = container(
        column![
            text("Быстрый запуск").size(14).color(TEXT),
            quick_item("instances", "Последняя сборка", last_instance, Page::Instances),
            quick_item("user", "Последний профиль", last_account, Page::Accounts),
        ]
        .spacing(13),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let status = container(
        row![
            container(icon("check", GREEN, 16.0))
                .width(36)
                .height(36)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|_| glass_style(Color::from_rgba(0.04, 0.24, 0.12, 0.72), EDGE)),
            column![
                text("Лаунчер готов").size(10).color(TEXT),
                text(format!("{} сборок · {} аккаунтов", app.data.instances.len(), app.data.accounts.len()))
                    .size(8)
                    .color(MUTED),
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
    icon_name: &'static str,
    title: &'static str,
    description: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    row![
        container(icon(icon_name, accent, 18.0))
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
    icon_name: &'static str,
    title: &'static str,
    subtitle: String,
    page: Page,
) -> Element<'static, Message> {
    button(
        row![
            container(icon(icon_name, CYAN, 16.0))
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
            icon("chevron-right", CYAN, 13.0),
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

fn version_choices_view(app: &App) -> Element<'_, Message> {
    let query = app.version_query.trim().to_ascii_lowercase();
    let matches: Vec<_> = app
        .version_choices
        .iter()
        .filter(|choice| {
            query.is_empty()
                || choice.id.to_ascii_lowercase().contains(&query)
                || choice.label.to_ascii_lowercase().contains(&query)
                || choice.kind.to_ascii_lowercase().contains(&query)
        })
        .take(14)
        .collect();

    let has_matches = !matches.is_empty();
    let mut choices = column![].spacing(4);
    for choice in matches {
        let active = app.instance_version == choice.id;
        let id = choice.id.clone();
        let label = choice.label.clone();
        choices = choices.push(
            button(text(label).size(9).color(if active { TEXT } else { MUTED }))
                .on_press(Message::VersionSelected(id))
                .width(Length::Fill)
                .padding([6, 9])
                .style(move |theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(
                        if active { PANEL_LIGHT } else { Color::TRANSPARENT }.into()
                    );
                    style.text_color = if active { CYAN } else { MUTED };
                    style.border = Border::default().rounded(7).width(1).color(
                        if active { EDGE_BRIGHT } else { EDGE }
                    );
                    style
                }),
        );
    }

    let content: Element<'_, Message> = if app.versions_busy {
        container(text("Загружаю каталог версий…").size(9).color(MUTED))
            .padding(8)
            .width(Length::Fill)
            .into()
    } else if let Some(error) = &app.versions_error {
        container(
            column![
                text(error.clone()).size(9).color(RED),
                text("Можно ввести ID версии вручную или повторить загрузку списка.")
                    .size(8)
                    .color(MUTED),
            ]
            .spacing(4),
        )
        .padding(8)
        .width(Length::Fill)
        .into()
    } else if app.version_choices.is_empty() {
        container(text("Каталог пока не загружен. Нажми «Обновить» или введи ID вручную.")
            .size(9).color(MUTED))
            .padding(8)
            .width(Length::Fill)
            .into()
    } else if !has_matches {
        container(text("Ничего не найдено. Попробуй другой запрос или введи ID вручную.")
            .size(9).color(MUTED))
            .padding(8)
            .width(Length::Fill)
            .into()
    } else {
        scrollable(choices)
            .height(Length::Fixed(132.0))
            .width(Length::Fill)
            .into()
    };

    container(content)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL_LIGHT, EDGE))
        .into()
}

fn instances_page(app: &App) -> Element<'_, Message> {
    let loaders: &[LoaderKind] = match app.platform {
        PlatformChoice::Java => &[
            LoaderKind::Vanilla,
            LoaderKind::Fabric,
            LoaderKind::Forge,
            LoaderKind::NeoForge,
            LoaderKind::Quilt,
        ],
        PlatformChoice::Horizon | PlatformChoice::HorizonOffline => &[
            LoaderKind::HorizonContent,
            LoaderKind::HorizonNative,
        ],
    };

    let mut loader_buttons = row![];
    for loader in loaders {
        loader_buttons = loader_buttons.push(loader_button(*loader, app.loader));
    }
    loader_buttons = loader_buttons.spacing(7);

    let form = container(
        column![
            row![
                text("Создать сборку").size(16).color(TEXT),
                Space::new().width(Length::Fill),
                status_chip("НОВЫЙ ПРОФИЛЬ", CYAN_DARK, CYAN),
            ]
            .align_y(Alignment::Center),
            text("Название").size(9).color(MUTED),
            text_input("Например, My Survival", &app.instance_name)
                .on_input(Message::InstanceNameChanged)
                .padding(10)
                .size(11),
            text(match app.platform {
                PlatformChoice::Java => "Версия Minecraft Java",
                PlatformChoice::Horizon => "Версия Horizon Runtime",
                PlatformChoice::HorizonOffline => "Версия Horizon Offline Runtime",
            }).size(9).color(MUTED),
            text_input(
                match app.platform {
                    PlatformChoice::Java => "ID версии: 1.21.1, b1.7.3, a1.0.16…",
                    PlatformChoice::Horizon => "ID сборки: stable или nightly-YYYY-MM-DD-commit",
                    PlatformChoice::HorizonOffline => "ID сборки offline fork-а или stable",
                },
                &app.instance_version,
            )
                .on_input(Message::InstanceVersionChanged)
                .padding(10)
                .size(11),
            row![
                text_input(
                    match app.platform {
                        PlatformChoice::Java => "Фильтр: release, snapshot, beta, alpha…",
                        PlatformChoice::Horizon => "Фильтр релизов Horizon…",
                        PlatformChoice::HorizonOffline => "Фильтр релизов Horizon Offline…",
                    },
                    &app.version_query,
                )
                    .on_input(Message::VersionQueryChanged)
                    .padding(9)
                    .size(10)
                    .width(Length::Fill),
                quiet_button("Обновить", Message::ReloadVersionCatalog),
            ]
            .spacing(8),
            version_choices_view(app),
            text("Платформа").size(9).color(MUTED),
            row![
                choice_button("Minecraft Java", app.platform == PlatformChoice::Java, Message::SelectPlatform(PlatformChoice::Java)),
                choice_button("Horizon", app.platform == PlatformChoice::Horizon, Message::SelectPlatform(PlatformChoice::Horizon)),
                choice_button("Horizon Offline", app.platform == PlatformChoice::HorizonOffline, Message::SelectPlatform(PlatformChoice::HorizonOffline)),
            ]
            .spacing(8),
            text("Загрузчик").size(9).color(MUTED),
            loader_buttons,
            primary_button("Создать сборку", Message::CreateInstance),
            text("Первый запуск автоматически загрузит нужные файлы игры, библиотеки и Java Runtime.")
                .size(9)
                .color(SUBTLE),
        ]
        .spacing(8),
    )
    .padding(18)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let mut list = column![
        row![
            column![
                text("Твоя библиотека").size(16).color(TEXT),
                text(format!("{} игровых профилей", app.data.instances.len())).size(9).color(MUTED),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10)
    .width(Length::Fill);

    if app.data.instances.is_empty() {
        list = list.push(
            container(
                column![
                    icon("instances", SUBTLE, 28.0),
                    text("Пока нет сборок").size(13).color(TEXT),
                    text("Создай профиль выше. Java и Horizon будут храниться независимо.")
                        .size(9)
                        .color(MUTED),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .padding(24)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE)),
        );
    } else {
        for instance in &app.data.instances {
            let selected = app.data.settings.selected_instance_id.as_deref() == Some(instance.id.0.as_str());
            list = list.push(instance_card(instance, selected));
        }
    }

    column![form, list]
        .spacing(16)
        .width(Length::Fill)
        .into()
}

fn loader_button(loader: LoaderKind, current: LoaderKind) -> Element<'static, Message> {
    let active = loader == current;
    button(text(loader_label(loader)).size(9).color(if active { TEXT } else { MUTED }))
        .on_press(Message::SelectLoader(loader))
        .padding([8, 10])
        .style(move |theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(if active { CYAN_DARK } else { PANEL_LIGHT }.into());
            style.text_color = if active { CYAN } else { MUTED };
            style.border = Border::default().rounded(10).width(1).color(if active { EDGE_BRIGHT } else { EDGE });
            style
        })
        .into()
}

fn choice_button(label: &'static str, active: bool, message: Message) -> Element<'static, Message> {
    button(text(label).size(10).color(if active { TEXT } else { MUTED }))
        .on_press(message)
        .padding([9, 12])
        .style(move |theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(if active { CYAN_DARK } else { PANEL_LIGHT }.into());
            style.text_color = if active { CYAN } else { MUTED };
            style.border = Border::default().rounded(11).width(1).color(if active { EDGE_BRIGHT } else { EDGE });
            style
        })
        .into()
}

fn instance_card(instance: &GameInstance, selected: bool) -> Element<'static, Message> {
    let platform = match instance.platform {
        GamePlatform::Java => "Minecraft Java",
        GamePlatform::Horizon => "Minecraft Horizon",
        GamePlatform::HorizonOffline => "Horizon Offline (неофициальная сборка)",
    };
    let state = match instance.state {
        InstanceState::NotInstalled => "Не установлена",
        InstanceState::Ready => "Готова",
        InstanceState::Broken => "Требует проверки",
        InstanceState::Updating => "Обновляется",
    };
    let account_id = instance.id.0.clone();

    container(
        row![
            container(icon(
                if instance.platform == GamePlatform::Java { "minecraft" } else { "horizon" },
                if instance.platform == GamePlatform::Java { GREEN } else { ORANGE },
                20.0,
            ))
            .width(44)
            .height(44)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_| glass_style(PANEL_LIGHT, EDGE)),
            column![
                text(instance.name.clone()).size(12).color(TEXT),
                text(format!("{} · {} · {}", platform, instance.game_version, loader_label(instance.loader)))
                    .size(9)
                    .color(MUTED),
                text(state).size(8).color(SUBTLE),
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            if selected {
                status_chip("АКТИВНА", CYAN_DARK, CYAN)
            } else {
                quiet_button("Выбрать", Message::SelectInstance(account_id.clone()))
            },
            button(icon("play", CYAN, 15.0))
                .on_press(Message::LaunchInstance(account_id.clone()))
                .width(36)
                .height(36)
                .style(|theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(CYAN_DARK.into());
                    style.text_color = CYAN;
                    style.border = Border::default().rounded(18).color(EDGE_BRIGHT);
                    style
                }),
            button(icon("trash", RED, 14.0))
                .on_press(Message::DeleteInstance(account_id))
                .width(34)
                .height(34)
                .style(button::text),
        ]
        .spacing(11)
        .align_y(Alignment::Center),
    )
    .padding(13)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn wardrobe_account(app: &App) -> Option<&AccountProfile> {
    app.wardrobe_account_id.as_ref()
        .and_then(|id| app.data.accounts.iter().find(|account| &account.id == id))
}

fn ensure_wardrobe_account(app: &mut App) {
    let current_exists = app.wardrobe_account_id.as_ref().is_some_and(|id| {
        app.data.accounts.iter().any(|account| &account.id == id)
    });
    if !current_exists {
        app.wardrobe_account_id = app.data.settings.selected_account_id.clone()
            .filter(|id| app.data.accounts.iter().any(|account| &account.id == id))
            .or_else(|| app.data.accounts.first().map(|account| account.id.clone()));
    }
}

fn refresh_skin_preview(app: &mut App) {
    app.wardrobe_preview = wardrobe_account(app)
        .and_then(|account| app.wardrobe_selected_skin_id.as_deref()
            .map(|id| (account.uuid.clone(), id.to_owned())))
        .and_then(|(uuid, id)| store::load_skin_pixels(&uuid, &id).ok());
}

fn refresh_wardrobe(app: &mut App) {
    let Some(account) = wardrobe_account(app) else {
        app.wardrobe_skins.clear();
        app.wardrobe_active_skin_id = None;
        app.wardrobe_selected_skin_id = None;
        app.rename_skin_name.clear();
        app.wardrobe_preview = None;
        return;
    };
    match store::load_skin_collection(&account.uuid) {
        Ok(collection) => {
            app.wardrobe_active_skin_id = collection.active_skin_id.clone();
            app.wardrobe_skins = collection.skins;
            if !app.wardrobe_selected_skin_id.as_ref().is_some_and(|id| {
                app.wardrobe_skins.iter().any(|skin| &skin.id == id)
            }) {
                app.wardrobe_selected_skin_id = app.wardrobe_active_skin_id.clone()
                    .or_else(|| app.wardrobe_skins.first().map(|skin| skin.id.clone()));
            }
            app.rename_skin_name = app.wardrobe_selected_skin_id.as_ref()
                .and_then(|id| app.wardrobe_skins.iter().find(|skin| &skin.id == id))
                .map(|skin| skin.name.clone())
                .unwrap_or_default();
            refresh_skin_preview(app);
        }
        Err(error) => {
            app.wardrobe_preview = None;
            app.notice = Some(error);
        }
    }
}

fn wardrobe_page(app: &App) -> Element<'_, Message> {
    let mut account_picker = row![].spacing(8).width(Length::Fill);
    let accounts: Vec<_> = app.data.accounts.iter().collect();

    if accounts.is_empty() {
        return column![
            section_header("ГАРДЕРОБ", "Скины для offline- и Microsoft-аккаунтов."),
            container(
                column![
                    icon("user", PURPLE, 28.0),
                    text("Нет игровых аккаунтов").size(14).color(TEXT),
                    text("Добавь offline-профиль или войди через Microsoft в разделе «Аккаунты».")
                        .size(10)
                        .color(MUTED),
                    primary_button("Перейти к аккаунтам", Message::Navigate(Page::Accounts)),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .padding(22)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE)),
        ]
        .spacing(14)
        .into();
    }

    for account in &accounts {
        let active = app.wardrobe_account_id.as_deref() == Some(account.id.as_str());
        let label = format!("{} · {}", account.name, account.kind.label());
        account_picker = account_picker.push(
            button(text(label).size(10).color(if active { CYAN } else { TEXT }))
                .on_press(Message::SelectWardrobeAccount(account.id.clone()))
                .padding([9, 13])
                .style(move |theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(if active { CYAN_DARK } else { PANEL_LIGHT }.into());
                    style.text_color = if active { CYAN } else { TEXT };
                    style.border = Border::default().rounded(10).width(1)
                        .color(if active { EDGE_BRIGHT } else { EDGE });
                    style
                }),
        );
    }

    let Some(account) = wardrobe_account(app) else {
        return column![
            section_header("ГАРДЕРОБ", "Скины для offline- и Microsoft-аккаунтов."),
            text("Выбери игровой аккаунт.").size(11).color(MUTED),
            account_picker,
        ]
        .spacing(12)
        .into();
    };
    let account_uuid = account.uuid.clone();

    let import_panel = container(
        column![
            row![
                icon("image", PURPLE, 18.0),
                text("Импортировать скин").size(14).color(TEXT),
                Space::new().width(Length::Fill),
                status_chip("PNG · ДО 2 МБ", PANEL_LIGHT, MUTED),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            text(if account.kind == AccountKind::Microsoft {
                "Для Microsoft-аккаунта скин можно отправить в официальный профиль Minecraft."
            } else {
                "Для Java GlowstoneCL применяет скин через Java-agent прямо в обычном клиенте — без Fabric/Forge-модов. Horizon Offline поддерживает скин встроенно."
            })
                .size(9)
                .color(MUTED),
            text_input("Название (необязательно, возьмём имя файла)", &app.skin_name)
                .on_input(Message::SkinNameChanged)
                .padding(10)
                .size(10),
            row![
                choice_button("Steve · Classic", app.skin_model == store::SkinModel::Classic,
                    Message::SkinModelChanged(store::SkinModel::Classic)),
                choice_button("Alex · Slim", app.skin_model == store::SkinModel::Slim,
                    Message::SkinModelChanged(store::SkinModel::Slim)),
                Space::new().width(Length::Fill),
                primary_button("Выбрать PNG", Message::PickSkinFile),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(9),
    )
    .padding(15)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let selected_skin = app.wardrobe_selected_skin_id.as_deref()
        .and_then(|id| app.wardrobe_skins.iter().find(|skin| skin.id == id));
    let preview_panel: Element<'_, Message> = if let (Some(texture), Some(skin)) =
        (app.wardrobe_preview.as_ref(), selected_skin) {
        container(
            column![
                row![
                    text("3D-ПРЕДПРОСМОТР").size(9).color(CYAN),
                    Space::new().width(Length::Fill),
                    text(skin.model.label()).size(9).color(MUTED),
                ].align_y(Alignment::Center),
                canvas::Canvas::new(crate::skin_preview::SkinPreview::new(
                    texture.clone(), skin.model, app.wardrobe_angle,
                ))
                    .width(Length::Fill)
                    .height(Length::Fixed(330.0)),
                row![
                    quiet_button("⟲ Влево", Message::RotateSkinPreview(-0.45)),
                    quiet_button("Вправо ⟳", Message::RotateSkinPreview(0.45)),
                    Space::new().width(Length::Fill),
                    text("Поворот модели").size(8).color(SUBTLE),
                ].align_y(Alignment::Center).spacing(8),
            ].spacing(8),
        )
        .padding(12)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE))
        .into()
    } else {
        container(
            column![
                text("3D-предпросмотр").size(11).color(TEXT),
                text("Выбери импортированный скин, чтобы увидеть модель персонажа.")
                    .size(9).color(MUTED),
            ].spacing(6),
        ).padding(14).width(Length::Fill).style(|_| glass_style(PANEL, EDGE)).into()
    };

    let active_preview: Element<'_, Message> = if let Some(active_id) = app.wardrobe_active_skin_id.as_deref() {
        if let Ok(path) = store::skin_image_path(&account_uuid, active_id) {
            if let Some(skin) = app.wardrobe_skins.iter().find(|skin| skin.id == active_id) {
                container(
                    row![
                        image::Image::new(image::Handle::from_path(path))
                            .width(Length::Fixed(112.0))
                            .height(Length::Fixed(112.0))
                            .content_fit(ContentFit::Contain),
                        column![
                            text("АКТИВНЫЙ СКИН").size(8).color(CYAN),
                            text(skin.name.clone()).size(14).color(TEXT),
                            text(skin.model.label()).size(9).color(MUTED),
                            text(if skin.legacy { "Legacy PNG · 64×32" } else { "PNG · 64×64" })
                                .size(8).color(SUBTLE),
                        ]
                        .spacing(6),
                        Space::new().width(Length::Fill),
                        status_chip("ВЫБРАН", CYAN_DARK, CYAN),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(15),
                )
                .padding(14)
                .width(Length::Fill)
                .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT))
                .into()
            } else {
                Space::new().height(0).into()
            }
        } else {
            Space::new().height(0).into()
        }
    } else {
        container(
            column![
                icon("image", SUBTLE, 24.0),
                text("Гардероб пока пуст").size(12).color(TEXT),
                text("Добавь PNG-файл, чтобы сохранить первый скин для этого аккаунта.")
                    .size(9).color(MUTED),
            ]
            .spacing(7)
            .align_x(Alignment::Center),
        )
        .padding(18)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE))
        .into()
    };

    let mut skin_list = column![
        row![
            column![
                text("Сохранённые скины").size(14).color(TEXT),
                text(format!("{} скинов в локальной коллекции", app.wardrobe_skins.len()))
                    .size(9).color(MUTED),
            ]
            .spacing(3),
            Space::new().width(Length::Fill),
            if app.wardrobe_selected_skin_id.is_some() {
                quiet_button("Переименовать", Message::RenameSelectedSkin)
            } else {
                status_chip("НЕТ ВЫБОРА", PANEL_LIGHT, MUTED)
            },
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(9)
    .width(Length::Fill);

    if app.wardrobe_skins.is_empty() {
        skin_list = skin_list.push(
            container(text("Здесь появятся импортированные скины.").size(10).color(MUTED))
                .padding(14)
                .width(Length::Fill)
                .style(|_| glass_style(PANEL, EDGE)),
        );
    } else {
        for skin in &app.wardrobe_skins {
            let skin_id = skin.id.clone();
            let active = app.wardrobe_active_skin_id.as_deref() == Some(skin.id.as_str());
            let selected = app.wardrobe_selected_skin_id.as_deref() == Some(skin.id.as_str());
            let preview = store::skin_image_path(&account_uuid, &skin.id).ok()
                .map(|path| image::Image::new(image::Handle::from_path(path))
                    .width(Length::Fixed(76.0))
                    .height(Length::Fixed(76.0))
                    .content_fit(ContentFit::Contain));
            let preview_element: Element<'_, Message> = match preview {
                Some(view) => view.into(),
                None => icon("image", SUBTLE, 22.0),
            };
            skin_list = skin_list.push(
                container(
                    row![
                        container(preview_element)
                            .width(84)
                            .height(84)
                            .align_x(Alignment::Center)
                            .align_y(Alignment::Center)
                            .style(|_| glass_style(PANEL_LIGHT, EDGE)),
                        column![
                            text(skin.name.clone()).size(11).color(TEXT),
                            text(skin.model.label()).size(8).color(MUTED),
                            text(if skin.legacy { "64×32 · legacy" } else { "64×64" })
                                .size(8).color(SUBTLE),
                            row![
                                quiet_button("Выбрать", Message::SelectSkin(skin_id.clone())),
                                if active {
                                    status_chip("АКТИВЕН", CYAN_DARK, CYAN)
                                } else {
                                    quiet_button("Сделать активным", Message::ActivateSkin(skin_id.clone()))
                                },
                                quiet_button(
                                    if account.kind == AccountKind::Microsoft {
                                        "Применить в Minecraft"
                                    } else {
                                        "Применить для игры"
                                    },
                                    Message::ApplySkin(skin_id.clone()),
                                ),
                            ]
                            .spacing(6),
                            row![
                                quiet_button("Steve", Message::SetSkinModel(skin_id.clone(), store::SkinModel::Classic)),
                                quiet_button("Alex", Message::SetSkinModel(skin_id.clone(), store::SkinModel::Slim)),
                                button(icon("trash", RED, 13.0))
                                    .on_press(Message::DeleteSkin(skin_id.clone()))
                                    .padding(8)
                                    .style(button::text),
                            ]
                            .spacing(6),
                        ]
                        .spacing(5),
                        Space::new().width(Length::Fill),
                        if selected { icon("check-circle", CYAN, 16.0) } else { Space::new().width(0).into() },
                    ]
                    .align_y(Alignment::Center)
                    .spacing(12),
                )
                .padding(10)
                .width(Length::Fill)
                .style(move |_| glass_style(
                    if selected { Color::from_rgba(0.12, 0.23, 0.34, 0.92) } else { PANEL },
                    if selected { EDGE_BRIGHT } else { EDGE },
                )),
            );
        }
    }

    column![
        section_header("ГАРДЕРОБ", format!("Персональные скины · {} · {}", account.name, account.kind.label())),
        account_picker,
        import_panel,
        preview_panel,
        active_preview,
        text_input("Новое название для выбранного скина", &app.rename_skin_name)
            .on_input(Message::RenameNameChanged)
            .padding(9)
            .size(10),
        skin_list,
        text(if account.kind == AccountKind::Microsoft {
            "Для Microsoft выбранный скин отправляется в официальный профиль Minecraft Services."
        } else {
            "Для Java используется OfflineSkinInjector — отдельный Java-agent, а не Minecraft-мод; он работает с Vanilla и загрузчиками, не добавляя файлы в mods. Поддержка старых и новых версий зависит от authlib игры (1.6.2 и старше не поддерживаются). Horizon Offline применяет скин встроенным механизмом."
        })
            .size(8)
            .color(SUBTLE),
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}

fn accounts_page(app: &App) -> Element<'_, Message> {
    let offline_form = container(
        column![
            row![
                icon("user", PURPLE, 18.0),
                text("Локальный offline-профиль").size(14).color(TEXT),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            text("Для одиночной игры, offline-серверов и Horizon Offline. Не даёт доступ к официальным online-серверам.")
                .size(9)
                .color(MUTED),
            row![
                text_input("Игровой ник (3–16 символов)", &app.offline_name)
                    .on_input(Message::OfflineNameChanged)
                    .padding(10)
                    .size(10)
                    .width(Length::Fill),
                primary_button("Добавить", Message::AddOfflineAccount),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let microsoft_form = container(
        column![
            row![
                icon("microsoft", CYAN, 18.0),
                text("Microsoft account").size(14).color(TEXT),
                Space::new().width(Length::Fill),
                status_chip("ОФИЦИАЛЬНЫЙ ВХОД", CYAN_DARK, CYAN),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text("Нажми вход — откроется браузер Microsoft с готовым запросом. По умолчанию используется официальный Client ID Minecraft Launcher; свой Client ID можно указать дополнительно.")
                .size(9)
                .color(MUTED),
            text("Дополнительный Microsoft Client ID (необязательно)").size(9).color(MUTED),
            row![
                text_input("Оставь пустым для официального входа", &app.client_id)
                    .on_input(Message::MicrosoftClientIdChanged)
                    .padding(10)
                    .size(10)
                    .width(Length::Fill),
                quiet_button("Сохранить", Message::SaveMicrosoftClientId),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                primary_button(
                    if app.auth_busy { "Подключение..." } else { "Войти через Microsoft" },
                    Message::StartMicrosoftLogin,
                ),
                if app.auth_busy {
                    icon("loader", CYAN, 16.0)
                } else {
                    icon("shield", GREEN, 16.0)
                },
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            microsoft_device_code(app),
            text("Свой Client ID — только для дополнительной конфигурации. Client Secret GlowstoneCL не использует и не запрашивает.")
                .size(8)
                .color(SUBTLE),
        ]
        .spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE));

    let mut account_list = column![
        row![
            column![
                text("Добавленные аккаунты").size(15).color(TEXT),
                text(format!("{} профилей", app.data.accounts.len())).size(9).color(MUTED),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(9)
    .width(Length::Fill);

    if app.data.accounts.is_empty() {
        account_list = account_list.push(
            container(
                column![
                    icon("user", SUBTLE, 26.0),
                    text("Аккаунтов пока нет").size(12).color(TEXT),
                    text("Добавь offline-профиль или войди через Microsoft.")
                        .size(9)
                        .color(MUTED),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .padding(20)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE)),
        );
    } else {
        for account in &app.data.accounts {
            let selected = app.data.settings.selected_account_id.as_deref() == Some(account.id.as_str());
            account_list = account_list.push(account_card(account, selected));
        }
    }

    column![microsoft_form, offline_form, account_list]
        .spacing(14)
        .width(Length::Fill)
        .into()
}

fn microsoft_device_code(app: &App) -> Element<'_, Message> {
    let Some(prompt) = &app.device_code else {
        return Space::new().height(0).into();
    };

    container(
        column![
            text("Подтверди вход").size(12).color(TEXT),
            text(&prompt.message).size(9).color(MUTED),
            text(&prompt.user_code).size(26).color(CYAN),
            row![
                quiet_button("Открыть страницу Microsoft ещё раз", Message::OpenMicrosoftVerification),
                if app.auth_busy {
                    status_chip("ОЖИДАНИЕ ПОДТВЕРЖДЕНИЯ", PANEL_LIGHT, CYAN)
                } else {
                    quiet_button("Повторить проверку", Message::PollMicrosoftLogin)
                },
            ]
            .spacing(8),
            text(&prompt.verification_uri).size(9).color(CYAN),
        ]
        .spacing(8)
        .align_x(Alignment::Start),
    )
    .padding(14)
    .width(Length::Fill)
    .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT))
    .into()
}

fn account_card(account: &AccountProfile, selected: bool) -> Element<'static, Message> {
    let (kind_label, accent) = match account.kind {
        AccountKind::Offline => ("OFFLINE", PURPLE),
        AccountKind::Microsoft => ("MICROSOFT", CYAN),
    };
    let id = account.id.clone();

    container(
        row![
            container(icon("user", accent, 18.0))
                .width(42)
                .height(42)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| glass_style(
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.14),
                    EDGE,
                )),
            column![
                text(account.name.clone()).size(11).color(TEXT),
                text(format!("{} · {}", kind_label, account.uuid)).size(8).color(MUTED),
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            if selected {
                status_chip("АКТИВЕН", CYAN_DARK, CYAN)
            } else {
                quiet_button("Выбрать", Message::SelectAccount(id.clone()))
            },
            button(icon("trash", RED, 14.0))
                .on_press(Message::DeleteAccount(id))
                .width(34)
                .height(34)
                .style(button::text),
        ]
        .align_y(Alignment::Center)
        .spacing(10),
    )
    .padding(12)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn settings_page(app: &App) -> Element<'_, Message> {
    let data_path = if cfg!(target_os = "linux") {
        "~/.local/share/GlowstoneCL/state.json"
    } else {
        "User data directory / GlowstoneCL / state.json"
    };

    column![
        settings_card("ВНЕШНИЙ ВИД", "Шрифт интерфейса", "На Linux используется Inter, на Windows — Segoe UI. Установи Inter через пакет inter-font в CachyOS/Arch."),
        settings_card("LINUX DISPLAY", "Нативный Wayland и X11", "GlowstoneCL поддерживает оба backend. В Wayland-сессии можно запускать приложение напрямую через Wayland без XWayland."),
        settings_card("ХРАНЕНИЕ", "Профили и сборки", data_path),
        settings_card("БЕЗОПАСНОСТЬ", "Microsoft refresh token", "Токены сохраняются через системное хранилище паролей. Если Secret Service недоступен, приложение покажет предупреждение."),
        container(
            column![
                text("Данные сейчас").size(12).color(TEXT),
                text(format!("{} сборок", app.data.instances.len())).size(10).color(MUTED),
                text(format!("{} аккаунтов", app.data.accounts.len())).size(10).color(MUTED),
            ]
            .spacing(6),
        )
        .padding(16)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ]
    .spacing(12)
    .into()
}

fn settings_card(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
) -> Element<'static, Message> {
    container(
        column![
            text(eyebrow).size(9).color(CYAN),
            text(title).size(14).color(TEXT),
            text(description).size(9).color(MUTED),
        ]
        .spacing(7),
    )
    .padding(16)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn catalog_page(app: &App) -> Element<'_, Message> {
    let selected = app.mod_target_instance.as_deref()
        .and_then(|id| app.data.instances.iter().find(|item| item.id.0 == id));
    let mut targets = row![text("СБОРКА").size(9).color(MUTED)].spacing(7).align_y(Alignment::Center);
    let mut target_count = 0usize;
    for instance in app.data.instances.iter().filter(|item| {
        item.platform == GamePlatform::Java && !matches!(item.loader, LoaderKind::Vanilla)
    }) {
        target_count += 1;
        let active = app.mod_target_instance.as_deref() == Some(instance.id.0.as_str());
        let label = format!("{} · {} · {}", instance.name, instance.game_version, loader_label(instance.loader));
        targets = targets.push(
            button(text(label).size(9).color(if active { CYAN } else { TEXT }))
                .on_press(Message::ModTargetSelected(instance.id.0.clone()))
                .padding([8, 11])
                .style(move |theme, status| {
                    let mut style = button::secondary(theme, status);
                    style.background = Some(if active { CYAN_DARK } else { PANEL_LIGHT }.into());
                    style.text_color = if active { CYAN } else { TEXT };
                    style.border = Border::default().rounded(9).width(1).color(if active { EDGE_BRIGHT } else { EDGE });
                    if matches!(status, button::Status::Hovered) {
                        style.background = Some(PANEL_LIGHT.into());
                    }
                    style
                })
        );
    }

    let current_profile = selected.map(|item| format!(
        "{} · Minecraft {} · {}",
        item.name, item.game_version, loader_label(item.loader)
    )).unwrap_or_else(|| "Сборка не выбрана".to_owned());

    let mut installed_list = column![].spacing(7);
    if app.installed_mods.is_empty() {
        installed_list = installed_list.push(
            text("Пока нет модов, установленных через каталог GlowstoneCL.")
                .size(9).color(SUBTLE)
        );
    } else {
        for item in &app.installed_mods {
            let project_id = item.project_id.clone();
            let target_id = app.mod_target_instance.clone().unwrap_or_default();
            installed_list = installed_list.push(
                container(
                    row![
                        column![
                            text(item.title.clone()).size(10).color(TEXT),
                            text(format!("{} · {}", item.version_number, item.file_name)).size(8).color(MUTED),
                            if item.dependency {
                                text("ОБЯЗАТЕЛЬНАЯ ЗАВИСИМОСТЬ").size(7).color(PURPLE)
                            } else {
                                text("MODRINTH").size(7).color(CYAN)
                            },
                        ].spacing(3),
                        Space::new().width(Length::Fill),
                        quiet_button("Удалить", Message::RemoveMod(target_id, project_id)),
                    ].align_y(Alignment::Center).spacing(8)
                )
                .padding(10)
                .width(Length::Fill)
                .style(|_| glass_style(PANEL_LIGHT, EDGE))
            );
        }
    }

    let mut results_list = column![].spacing(9);
    for project in &app.mod_results {
        let installed = app.installed_mods.iter().any(|item| item.project_id == project.project_id);
        let project_id = project.project_id.clone();
        let project_name = project.title.clone();
        let installing = app.mods_busy_project.as_deref() == Some(project.project_id.as_str());
        let button_label = if installing { "Загрузка…" } else { "Установить" };
        let action = if installed {
            Message::Info("Этот мод уже установлен в выбранную сборку.")
        } else if installing {
            Message::Info("Этот мод уже загружается.")
        } else {
            Message::InstallMod(project_id)
        };
        let versions = project.versions.iter().take(5).cloned().collect::<Vec<_>>().join(" · ");
        let versions = if versions.is_empty() { "Совместимая версия".to_owned() } else { versions };
        results_list = results_list.push(
            container(
                row![
                    column![
                        text(project_name).size(12).color(TEXT),
                        text(format!("от {} · {} загрузок", project.author, format_downloads(project.downloads)))
                            .size(8).color(CYAN),
                        text(project.description.clone()).size(9).color(MUTED),
                        text(versions).size(8).color(SUBTLE),
                    ].spacing(5).width(Length::Fill),
                    if installed {
                        status_chip("УСТАНОВЛЕН", CYAN_DARK, CYAN)
                    } else {
                        primary_button(button_label, action)
                    },
                ].spacing(12).align_y(Alignment::Center)
            )
            .padding(13)
            .width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE))
        );
    }

    let mut content = column![
        section_header("MODRINTH · КАТАЛОГ МОДОВ", "Поиск и установка модов Minecraft Java с учётом версии игры и загрузчика."),
        container(
            column![
                row![
                    text_input("Название мода: Sodium, JEI, Create…", &app.search)
                        .on_input(Message::SearchChanged)
                        .on_submit(Message::SearchMods)
                        .padding(10)
                        .size(10)
                        .width(Length::Fill),
                    primary_button("Найти", Message::SearchMods),
                ].spacing(8),
                targets,
                text(format!("Целевая сборка: {current_profile}")).size(9).color(MUTED),
                text("Моды будут установлены только в эту сборку. Обязательные зависимости Modrinth скачиваются автоматически.")
                    .size(8).color(SUBTLE),
            ].spacing(9)
        )
        .padding(14)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ].spacing(12).width(Length::Fill);

    if target_count == 0 {
        content = content.push(
            container(
                column![
                    icon("puzzle", PURPLE, 24.0),
                    text("Сначала нужна Java-сборка с загрузчиком модов").size(13).color(TEXT),
                    text("Создай сборку Minecraft Java и выбери Fabric, Forge, NeoForge или Quilt. После этого вернись в каталог.")
                        .size(9).color(MUTED),
                    primary_button("Создать сборку", Message::Navigate(Page::Instances)),
                ].spacing(8)
            )
            .padding(18).width(Length::Fill)
            .style(|_| glass_style(PANEL, EDGE))
        );
    } else {
        content = content.push(
            column![
                row![
                    text("УСТАНОВЛЕННЫЕ МОДЫ").size(9).color(CYAN),
                    Space::new().width(Length::Fill),
                    text(format!("{}", app.installed_mods.len())).size(9).color(MUTED),
                ].align_y(Alignment::Center),
                installed_list,
            ].spacing(8)
        );
        content = content.push(
            row![
                text("РЕЗУЛЬТАТЫ ПОИСКА").size(9).color(CYAN),
                Space::new().width(Length::Fill),
                quiet_button("Обновить", Message::SearchMods),
            ].align_y(Alignment::Center)
        );
        if app.mods_busy {
            content = content.push(text("Обращаюсь к Modrinth…").size(9).color(MUTED));
        }
        if let Some(error) = &app.mods_error {
            content = content.push(
                container(text(error.clone()).size(9).color(ORANGE))
                    .padding(11).width(Length::Fill)
                    .style(|_| glass_style(PANEL, EDGE))
            );
        }
        if app.mod_results.is_empty() && !app.mods_busy && app.mods_error.is_none() {
            content = content.push(text("Ничего не найдено. Попробуй изменить запрос.").size(9).color(MUTED));
        }
        content = content.push(results_list);
    }

    if let Some(notice) = &app.notice {
        content = content.push(
            container(text(notice.clone()).size(9).color(TEXT))
                .padding(10).width(Length::Fill)
                .style(|_| glass_style(CYAN_DARK, EDGE_BRIGHT))
        );
    }
    content.into()
}

fn format_downloads(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1} млн", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1} тыс.", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn source_card(
    name: &'static str,
    description: &'static str,
    status: &'static str,
    accent: Color,
) -> Element<'static, Message> {
    container(
        column![
            text(name).size(11).color(accent),
            text(description).size(11).color(TEXT),
            status_chip(status, PANEL_LIGHT, MUTED),
        ]
        .spacing(8),
    )
    .padding(17)
    .width(Length::Fill)
    .style(|_| glass_style(PANEL, EDGE))
    .into()
}

fn modpacks_page() -> Element<'static, Message> {
    column![
        section_header("МОДПАКИ", "Импортируй готовые сборки или управляй собственными."),
        container(
            column![
                icon("package", PURPLE, 26.0),
                text("Модпаки появятся здесь").size(15).color(TEXT),
                text("Планируется импорт Modrinth .mrpack и CurseForge ZIP. Установка будет создавать отдельный игровой экземпляр.")
                    .size(10)
                    .color(MUTED),
                primary_button("Перейти к моим сборкам", Message::Navigate(Page::Instances)),
            ]
            .spacing(9),
        )
        .padding(20)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ]
    .spacing(14)
    .into()
}

fn downloads_page() -> Element<'static, Message> {
    column![
        section_header("ОЧЕРЕДЬ ЗАДАЧ", "Загрузка игры, библиотек, модов и обновлений."),
        container(
            column![
                icon("download", CYAN, 26.0),
                text("Нет активных загрузок").size(15).color(TEXT),
                text("Когда будет подключён установщик, здесь появятся прогресс, скорость и проверка целостности.")
                    .size(10)
                    .color(MUTED),
            ]
            .spacing(9),
        )
        .padding(20)
        .width(Length::Fill)
        .style(|_| glass_style(PANEL, EDGE)),
    ]
    .spacing(14)
    .into()
}

fn section_header(
    title: impl Into<String>,
    subtitle: impl Into<String>,
) -> Element<'static, Message> {
    row![
        column![
            text(title.into()).size(9).color(CYAN),
            text(subtitle.into()).size(11).color(MUTED),
        ]
        .spacing(3),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn status_chip(
    label: &'static str,
    background: Color,
    foreground: Color,
) -> Element<'static, Message> {
    container(text(label).size(8).color(foreground))
        .padding([6, 9])
        .style(move |_| glass_style(background, EDGE))
        .into()
}

fn primary_button(label: &'static str, message: Message) -> Element<'static, Message> {
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

fn tonal_button(label: &'static str, message: Message) -> Element<'static, Message> {
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

fn quiet_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(9).color(TEXT))
        .on_press(message)
        .padding([10, 14])
        .style(|theme, status| {
            let mut style = button::secondary(theme, status);
            style.background = Some(PANEL_LIGHT.into());
            style.text_color = TEXT;
            style.border = Border::default().rounded(11).width(1).color(EDGE);
            style
        })
        .into()
}

fn loader_label(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Vanilla => "Vanilla",
        LoaderKind::Forge => "Forge",
        LoaderKind::NeoForge => "NeoForge",
        LoaderKind::Fabric => "Fabric",
        LoaderKind::Quilt => "Quilt",
        LoaderKind::HorizonContent => "Horizon Content",
        LoaderKind::HorizonNative => "Horizon Native",
    }
}

fn icon(
    name: &'static str,
    color: Color,
    size: f32,
) -> Element<'static, Message> {
    let stroke = format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
    );

    let markup = match name {
        "diamond" => format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                <defs>
                    <linearGradient id="crystal" x1="0" y1="0" x2="1" y2="1">
                        <stop offset="0" stop-color="#B8FAFF"/>
                        <stop offset="0.48" stop-color="#43D6F2"/>
                        <stop offset="1" stop-color="#2378D8"/>
                    </linearGradient>
                </defs>
                <path d="M12 1.8 21 7v10l-9 5.2L3 17V7l9-5.2Z" fill="url(#crystal)" stroke="{stroke}" stroke-width="1.2"/>
                <path d="m3 7 9 5.2L21 7M12 12.2v10M7.5 4.4 12 12.2l4.5-7.8" fill="none" stroke="#E8FDFF" stroke-width="1.05" stroke-linejoin="round" opacity=".9"/>
            </svg>"##
        ),
        "minecraft" => format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                <path d="m12 1.8 10 5.6-10 5.7L2 7.4 12 1.8Z" fill="#63B94A" stroke="{stroke}" stroke-width="1.1" stroke-linejoin="round"/>
                <path d="M2 7.4 12 13.1v9L2 16.5V7.4Z" fill="#95602F" stroke="{stroke}" stroke-width="1.1" stroke-linejoin="round"/>
                <path d="m22 7.4-10 5.7v9l10-5.6V7.4Z" fill="#67401F" stroke="{stroke}" stroke-width="1.1" stroke-linejoin="round"/>
                <path d="m2 7.4 3.1 1.8 2.2-1.1 2.3 1.3 2.4-1.2v4.9L2 7.4Z" fill="#8CD65A" opacity=".9"/>
                <path d="m12 13.1 10-5.7M12 13.1v9" fill="none" stroke="#D6F3C0" stroke-width=".7" opacity=".65"/>
            </svg>"##
        ),
        _ => {
            let path = match name {
                "home" => "M3 10.5 12 3l9 7.5V21h-6v-6H9v6H3V10.5Z",
                "instances" => "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z",
                "puzzle" => "M8 4a2 2 0 1 1 4 0v2h4v4h2a2 2 0 1 1 0 4h-2v4h-4v-2a2 2 0 1 0-4 0v2H4v-4h2a2 2 0 1 0 0-4H4V6h4V4Z",
                "package" => "m12 3 9 5v8l-9 5-9-5V8l9-5Z M3 8l9 5 9-5 M12 13v8 M7.5 5.5l9 5",
                "download" => "M12 3v12m0 0 4-4m-4 4-4-4 M4 18v3h16v-3",
                "user" => "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M4 21a8 8 0 0 1 16 0",
                "shirt" => "m16 4 5 3-2 5-3-1v9H8v-9l-3 1-2-5 5-3c1 2 2 3 4 3s3-1 4-3Z",
                "image" => "M4 4h16v16H4z M4 16l5-5 4 4 3-3 4 4 M9 9h.01",
                "check-circle" => "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z M8 12l3 3 5-6",
                "settings" => "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-1.8 3.1-.2-.1a1.7 1.7 0 0 0-1.9.2l-.1.1h-3.6l-.1-.2a1.7 1.7 0 0 0-1.6-1 1.7 1.7 0 0 0-1.1.4l-.2.1-3.1-1.8.1-.2a1.7 1.7 0 0 0-.2-1.9l-.1-.1v-3.6l.2-.1a1.7 1.7 0 0 0 .6-2.7l-.1-.2 1.8-3.1.2.1a1.7 1.7 0 0 0 1.9-.2l.1-.1h3.6l.1.2a1.7 1.7 0 0 0 2.7.6l.2-.1 3.1 1.8-.1.2a1.7 1.7 0 0 0 .2 1.9l.1.1V15Z",
                "search" => "M10.5 18a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15Z M16 16l5 5",
                "community" => "M16 21v-2a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v2 M9.5 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M20 8v6 M17 11h6",
                "discord" => "M7 7c3-2 7-2 10 0l2 10c-3 2-11 2-14 0L7 7Z M9 12h.01 M15 12h.01 M8 8 6 5 M16 8l2-3",
                "globe" => "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M2 12h20 M12 2a15 15 0 0 1 0 20 M12 2a15 15 0 0 0 0 20",
                "chevron-down" => "m6 9 6 6 6-6",
                "chevron-right" => "m9 18 6-6-6-6",
                "arrow-right" => "M4 12h16 M13 5l7 7-7 7",
                "play" => "m8 5 12 7-12 7V5Z",
                "plus" => "M12 4v16 M4 12h16",
                "trash" => "M4 7h16 M10 11v6 M14 11v6 M6 7l1 14h10l1-14 M9 7V4h6v3",
                "check" => "m5 12 4 4L19 6",
                "shield" => "M12 3 20 6v5c0 5-3.5 8.5-8 10-4.5-1.5-8-5-8-10V6l8-3Z M9 12l2 2 4-4",
                "microsoft" => "M3 3h8v8H3z M13 3h8v8h-8z M3 13h8v8H3z M13 13h8v8h-8z",
                "loader" => "M20 12a8 8 0 1 1-2.3-5.7",
                "java" => "M6 3c-2 4 3 5 0 9 M12 2c-2 4 3 6 0 10 M18 3c-2 4 3 5 0 9 M5 15h14l-2 5H7l-2-5Z M3 22h18",
                "forge" | "curseforge" => "M12 2c3 4-2 6 1 9 2-2 4-3 4-6 4 5 5 10 1 14-4 4-11 3-13-2-2-4 0-9 4-12 0 4 1 5 3 6 1-3-1-5 0-9Z",
                "horizon" => "M3 18 8 6l4 8 4-8 5 12 M3 18h18",
                "modrinth" => "M12 2 22 12 12 22 2 12 12 2Z M12 6 18 12 12 18 6 12 12 6Z",
                _ => "M4 4h16v16H4z",
            };
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{stroke}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="{path}"/></svg>"##
            )
        }
    };

    svg::Svg::new(svg::Handle::from_memory(markup.into_bytes()))
        .width(size)
        .height(size)
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
