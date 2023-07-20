/*
 * Amethyst Launcher Core
 * Copyright (C) 2023 Broken-Deer <old_driver__@outlook.com> and contributors
 *
 * This program is free software, you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

// use std::str::FromStr;

// use anyhow::anyhow;

// use crate::core::{folder::MinecraftLocation, version::MinecraftVersion};

// pub struct ForgeInstallOptions {}

// impl Default for ForgeInstallOptions {
//     fn default() -> Self {
//         Self {  }
//     }
// }

// // pub fn installation_preparation(options: Option<ForgeInstallOptions>) {}

// pub fn install(forge_version: &str,minecraft_version: &str, minecraft_location: MinecraftLocation) {
//     let minor_version = if let MinecraftVersion::Release(_, minor, _) = MinecraftVersion::from_str(minecraft_version).unwrap() {
//         minor
//     } else {
//         // return Err(anyhow!(""));
//         panic!("")
//     };

//     let forge_version = if minor_version >= 7 && minor_version <= 8 {
//         format!("{mc}-{forge}-{mc}", mc=minecraft_version, forge=forge_version)
//     } else {
//         format!("{mc}-{forge}", mc=minecraft_version, forge=forge_version)
//     };
// }

use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

use anyhow::Result;
use regex::Regex;
use reqwest::Response;
use zip::ZipArchive;

use crate::{
    core::{
        folder::MinecraftLocation,
        task::TaskEventListeners,
        version::{LibraryDownload, MinecraftVersion},
        HTTP_CLIENT,
    },
    install::{
        forge::{
            install_profile::{InstallProfile, InstallProfileLegacy},
            legacy_install::install_legacy_forge_from_zip,
            new_install::unpack_forge_installer,
        },
        install_vanilla_game, quick_install_dependencies,
    },
    utils::{
        download::{download, Download},
        unzip::filter_entries,
    },
};

use super::*;

const DEFAULT_FORGE_MAVEN: &str = "http://files.minecraftforge.net/maven";

// todo: 使用 Steve-xmh/forge-install-bootstrapper 修复新版forge安装

async fn download_forge_installer(
    required_version: RequiredVersion,
    minecraft: &MinecraftLocation,
    _options: &Option<InstallForgeOptions>,
) -> Result<(String, Response)> {
    let url = find_download_link(&required_version.version, &required_version.mcversion).await?;
    let path = url.replace("https://maven.minecraftforge.net/", "");
    // let forge_maven_path = path.replace("/maven", "").replace("maven", "");
    let sha1 = match &required_version.installer {
        Some(installer) => match &installer.sha1 {
            Some(sha1) => String::from(sha1),
            _ => String::new(),
        },
        _ => String::new(),
    };
    let library = LibraryDownload {
        url,
        path,
        size: 0,
        sha1,
    };
    println!("{:#?}", library);
    let file_path = minecraft
        .get_library_by_path(&library.path)
        .to_str()
        .ok_or(std::io::Error::from(std::io::ErrorKind::NotFound))
        .unwrap()
        .to_string();
    let response = download(Download {
        url: library.url,
        file: file_path.clone(),
        sha1: None,
    })
    .await;
    Ok((file_path, response.unwrap()))
}

async fn walk_forge_installer_entries<R: Read + io::Seek>(
    mut zip: ZipArchive<R>,
    forge_version: &str,
) -> ForgeInstallerEntries {
    let entries = vec![
        format!(
            "maven/net/minecraftforge/forge/{}/forge-{}.jar",
            forge_version, forge_version
        ),
        format!(
            "maven/net/minecraftforge/forge/{}/forge-{}-universal.jar",
            forge_version, forge_version
        ),
        "data/client.lzma".to_string(),
        "data/server.lzma".to_string(),
        "install_profile.json".to_string(),
        "version.json".to_string(),
        format!("forge-{}-universal.jar", forge_version),
        "data/run.sh".to_string(),
        "data/run.bat".to_string(),
        "data/unix_args.txt".to_string(),
        "data/unix_jvm_args".to_string(),
        "data/win_args".to_string(),
    ];
    let filted_entries = filter_entries(&mut zip, &entries);
    let get_content = move |index: usize| -> Option<Entry> {
        match filted_entries.get(entries.clone().get(index).unwrap()) {
            None => None,
            Some(value) => Some(value.clone()),
        }
    };
    ForgeInstallerEntries {
        forge_jar: get_content(0),
        forge_universal_jar: get_content(1),
        client_lzma: get_content(2),
        server_lzma: get_content(3),
        install_profile_json: get_content(4),
        version_json: get_content(5),
        legacy_universal_jar: get_content(6),
        run_sh: get_content(7),
        run_bat: get_content(8),
        unix_args: get_content(9),
        user_jvm_args: get_content(10),
        win_args: get_content(11),
    }
}

pub async fn install_forge(
    version: RequiredVersion,
    minecraft: MinecraftLocation,
    options: Option<InstallForgeOptions>,
) -> Result<()> {
    let mcversion: Vec<_> = version.mcversion.split(".").collect();
    let minor = *mcversion.get(1).unwrap();
    let minor_version = minor.parse::<i32>().unwrap();
    let patch = mcversion.get(2);
    let forge_version = if minor_version >= 7 && minor_version <= 8 {
        match patch {
            Some(patch) => {
                if (&version.version == "10.12.2.1154"
                    || &version.version == "10.12.2.1155"
                    || &version.version == "10.12.2.1161")
                    && *patch == "2"
                {
                    format!(
                        "{}-{}-mc{}",
                        version.mcversion,
                        version.version,
                        version.mcversion.replace(".", "")
                    )
                } else {
                    format!(
                        "{mc}-{forge}-{mc}",
                        mc = version.mcversion,
                        forge = version.version
                    )
                }
            }
            None => format!(
                "{mc}-{forge}-{mc}",
                mc = version.mcversion,
                forge = version.version
            ),
        }
    } else {
        format!("{}-{}", version.mcversion, version.version)
    };

    let (installer_jar_path, _installer_jar) =
        download_forge_installer(version, &minecraft, &options)
            .await
            .unwrap();
    println!("{}", installer_jar_path);

    let file = Path::new(&installer_jar_path);
    if file.exists() {
        let file_size = std::fs::metadata(file)?.len();
        if file_size == 0 {
            eprintln!("ZIP file is empty");
            return Err(anyhow::anyhow!(""));
        }
    } else {
        eprintln!("ZIP file does not exist");
        return Err(anyhow::anyhow!(""));
    }

    thread::sleep(Duration::from_secs(1));
    let installer_jar = ZipArchive::new(File::open(&installer_jar_path).unwrap()).unwrap();

    let entries = walk_forge_installer_entries(installer_jar, &forge_version).await;
    let mut installer_jar = ZipArchive::new(File::open(&installer_jar_path).unwrap()).unwrap();

    let install_profile_json = match &entries.install_profile_json {
        None => panic!("Bad forge installer jar!"),
        Some(data) => String::from_utf8(data.content.clone()).unwrap(),
    };
    println!("{}", install_profile_json);
    let forge_type = if let Some(_) = &entries.install_profile_json {
        if let Some(_) = entries.version_json {
            ForgeType::New
        } else if let Some(_) = &entries.legacy_universal_jar {
            ForgeType::Legacy
        } else {
            ForgeType::Bad
        }
    } else {
        ForgeType::Bad
    };
    match forge_type {
        ForgeType::New => {
            let profile: InstallProfile = serde_json::from_str(&install_profile_json).unwrap();
            let _version_id = unpack_forge_installer(
                &mut installer_jar,
                entries,
                &forge_version,
                minecraft,
                PathBuf::from_str((&installer_jar_path).as_ref()).unwrap(),
                profile,
                options,
            )
            .await;
        }
        ForgeType::Legacy => {
            let profile: InstallProfileLegacy =
                serde_json::from_str(&install_profile_json).unwrap();
            let entries = ForgeLegacyInstallerEntriesPatten {
                install_profile_json: entries
                    .install_profile_json
                    .ok_or(std::io::Error::from(std::io::ErrorKind::NotFound))
                    .unwrap(),
                legacy_universal_jar: entries
                    .legacy_universal_jar
                    .ok_or(std::io::Error::from(std::io::ErrorKind::NotFound))
                    .unwrap(),
            };
            install_legacy_forge_from_zip(entries, profile, minecraft, options)
                .await
                .unwrap();
        }
        ForgeType::Bad => panic!("Bad forge installer jar!"),
    }

    Ok(())
}

pub async fn find_download_link(forge_version: &str, minecraft_version: &str) -> Result<String> {
    let document_url = format!(
        "https://files.minecraftforge.net/net/minecraftforge/forge/index_{minecraft_version}.html"
    );
    let document = HTTP_CLIENT
        .get(document_url)
        .send()
        .await?
        .text()
        .await?
        .replace("\r\n", "\n");
    let minecraft_version = if let MinecraftVersion::Release(_, minor, _) =
        MinecraftVersion::from_str(minecraft_version).unwrap()
    {
        minor
    } else {
        return Err(anyhow::anyhow!(""));
    };
    let document_split = document
        .split("\n")
        .filter(|x| {
            let installer = if minecraft_version < 3 {
                "clie"
            } else if minecraft_version < 13 {
                "uni"
            } else {
                "ins"
            };
            x.contains("href=")
                && (x.contains(installer))
                && x.contains(&format!("{forge_version}-"))
                && !x.contains("adfoc.us")
                && !x.contains("mdk")
                && !x.contains("changelog.txt")
                && !x.contains("info")
        })
        .collect::<Vec<&str>>();
    // println!("{document_split:?}");
    if document_split.len() != 1 {
        return Err(anyhow::anyhow!("cannot find download link"));
    }
    let link_element = document_split
        .get(0)
        .ok_or(anyhow::anyhow!("cannot find download link"))?;

    let regex = Regex::new(r#"href="(.*?)""#)?;
    let link = regex
        .captures(link_element)
        .ok_or(anyhow::anyhow!("cannot find download link"))?
        .get(1)
        .ok_or(anyhow::anyhow!("cannot find download link"))?
        .as_str();
    Ok(link.to_string())
}

#[tokio::test]
async fn test() {
    // use crate::install::TaskEventListeners;
    // let miencraft_version = "1.20.1";
    // let minecraft_location = MinecraftLocation::new("test");
    // install_vanilla_game(
    //     miencraft_version,
    //     minecraft_location,
    //     TaskEventListeners::default(),
    // )
    // .await
    // .unwrap();
    // let a = find_download_link("25.0.160", "1.13.2").await.unwrap();
    // println!("{a}");
    // install_forge(
    //     RequiredVersion {
    //         installer: None,
    //         mcversion: "1.13.2".to_string(),
    //         version: "25.0.160".to_string(),
    //     },
    //     MinecraftLocation::new("test"),
    //     None,
    // )
    // .await
    // .unwrap();
    // install_vanilla_game(
    //     "1.13.2",
    //     MinecraftLocation::new("test"),
    //     TaskEventListeners::default(),
    // ).await.unwrap();
    quick_install_dependencies(
        "1.13.2-forge-25.0.160",
        MinecraftLocation::new("test"),
        TaskEventListeners::default(),
    )
    .await
    .unwrap();
    // let version = tokio::fs::read_to_string(minecraft_location.get_version_json("1.7.2"))
    //     .await
    //     .unwrap();
    // install_forge(
    //     RequiredVersion {
    //         installer: None,
    //         mcversion: "1.7.2".to_string(),
    //         version: "10.12.2.1161".to_string(),
    //     },
    //     minecraft_location,
    //     Some(InstallForgeOptions {
    //         maven_host: None,
    //         libraries_download_concurrency: None,
    //         inherits_from: Some("1.7.2".to_string()),
    //         version_id: Some("1.7.2-forge-10.12.2.1161".to_string()),
    //         java: None,
    //     }),
    // )
    // .await
    // .unwrap();
    // use crate::core::PlatformInfo;
    // use crate::core::version::Version;
    // let minecraft = MinecraftLocation::new("test");
    // let version = minecraft.get_version_json("1.7.2-forge-10.12.2.1161");
    // let version = tokio::fs::read_to_string(version).await.unwrap();
    // let platform = PlatformInfo::new().await;
    // let version = Version::from_str(&version).unwrap();
    // let version = version.parse(&minecraft, &platform).await.unwrap();
    // println!("{}", serde_json::to_string(&version).unwrap());
    // use crate::install::quick_install_dependencies;
    // let listener = TaskEventListeners::default();
    // let minecraft = MinecraftLocation::new("test");
    // quick_install_dependencies("1.7.2-forge-10.12.2.1161", minecraft, listener)
    //     .await
    //     .unwrap();
}
