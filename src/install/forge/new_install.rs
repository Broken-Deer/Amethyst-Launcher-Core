/*
 * Magical Launcher Core
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

use std::{
    collections::HashMap,
    io::{self, Read},
    path::PathBuf,
};

use serde_json::Value;
use tokio::fs::{self, create_dir_all};
use zip::ZipArchive;

use crate::{
    core::{folder::MinecraftLocation, version::Version},
    utils::unzip::decompression_files,
};

use super::{
    install_profile::{InstallProfile, InstallProfileData},
    *,
};

pub(super) async fn unpack_forge_installer<R: Read + io::Seek>(
    zip: &mut ZipArchive<R>,
    entries: ForgeInstallerEntries,
    forge_version: &String,
    minecraft: MinecraftLocation,
    jar_path: PathBuf,
    profile: InstallProfile,
    options: Option<InstallForgeOptions>,
) -> String {
    let version_json_raw = entries.version_json.unwrap().content;
    let mut version_json: Value =
        serde_json::from_str((&String::from_utf8(version_json_raw).unwrap()).as_ref()).unwrap();

    //  apply override for inheritsFrom
    if let Some(options) = options {
        if let Some(id) = options.version_id {
            version_json["id"] = Value::String(id);
        }
        if let Some(inherits_from) = options.inherits_from {
            version_json["inheritsFrom"] = Value::String(inherits_from);
        }
    }

    //   resolve all the required paths
    let root_path = minecraft.root.clone();

    let version_json_path =
        root_path.join(format!("{}.json", version_json["id"].as_str().unwrap()));
    let install_json_path = root_path.join("install_profile.json");

    let data_root = jar_path.parent().unwrap().to_path_buf();

    let mut decompression_tasks: Vec<(String, PathBuf)> = Vec::new();

    create_dir_all(version_json_path.parent().unwrap())
        .await
        .unwrap();

    if let Some(_) = entries.forge_universal_jar {
        decompression_tasks.push((
            format!(
                "maven/net/minecraftforge/forge/{}/forge-{}-universal.jar",
                forge_version, forge_version
            ),
            minecraft.libraries.clone().join(format!(
                "maven/net/minecraftforge/forge/{}/forge-{}-universal.jar",
                forge_version, forge_version
            )),
        ));
    }
    let mut profile_data;
    if let Some(h) = profile.data.clone() {
        profile_data = h;
    } else {
        profile_data = HashMap::new();
    }

    let installer_maven = format!("net.minecraftforge:forge:{forge_version}:installer");
    let profile_data_installer = InstallProfileData {
        client: Some(format!("[{installer_maven}]")),
        server: Some(format!("[{installer_maven}]")),
    };
    profile_data.insert("INSTALLER".to_string(), profile_data_installer);

    let path = &format!("net/minecraftforge/forge/{forge_version}/forge-{forge_version}.jar");
    if let Some(server_lzma) = entries.server_lzma {
        // forge version and mavens, compatible with twitch api
        let server_maven = format!("net.minecraftforge:forge:{forge_version}:serverdata@lzma");
        // override forge bin patch location
        profile_data.insert(
            "BINPATCH".to_string(),
            InstallProfileData {
                client: None,
                server: Some(format!("[{server_maven}]")),
            },
        );

        let server_bin_path = minecraft.libraries.join(path);
        decompression_tasks.push((server_lzma.name.clone(), server_bin_path));
    }

    if let Some(client_lzma) = entries.client_lzma {
        //forge version and mavens, compatible with twitch api
        let client_maven = format!("net.minecraftforge:forge:{forge_version}:clientdata@lzma");
        //override forge bin patch location
        let mut server = String::new();
        let binpatch = profile_data.get("BINPATCH");
        if let Some(b) = binpatch {
            if let Some(s) = b.server.clone() {
                server = s;
            }
        }
        profile_data.insert(
            "BINPATCH".to_string(),
            InstallProfileData {
                client: Some(format!("[{client_maven}]]")),
                server: Some(server),
            },
        );

        let client_bin_path = minecraft.libraries.join(format!(
            "net/minecraftforge/forge/{forge_version}/forge-{forge_version}.jar"
        ));
        decompression_tasks.push((client_lzma.name.clone(), client_bin_path));
    }

    if let Some(forge_jar) = entries.forge_jar {
        let file_name = entries.forge_universal_jar.unwrap().name;
        fs::write(
            minecraft.get_library_by_path(&file_name[file_name.find("/").unwrap() + 1..]),
            forge_jar.content,
        )
        .await
        .unwrap();
    }

    let unpack_data = |entry: Entry| async {
        let path = data_root.clone().join(entry.name);
        create_dir_all(path.parent().unwrap()).await.unwrap();
        fs::write(path, entry.content).await.unwrap();
    };

    if let Some(run_bat) = entries.run_bat {
        unpack_data(run_bat).await;
    }
    if let Some(run_sh) = entries.run_sh {
        unpack_data(run_sh).await;
    }
    if let Some(win_args) = entries.win_args {
        unpack_data(win_args).await;
    }
    if let Some(unix_args) = entries.unix_args {
        unpack_data(unix_args).await;
    }
    if let Some(unix_jvm_args) = entries.user_jvm_args {
        unpack_data(unix_jvm_args).await;
    }

    create_dir_all(install_json_path.parent().unwrap())
        .await
        .unwrap();
    fs::write(
        install_json_path,
        serde_json::to_string_pretty(&profile).unwrap(),
    )
    .await
    .unwrap();

    create_dir_all(version_json_path.parent().unwrap())
        .await
        .unwrap();
    fs::write(
        version_json_path,
        serde_json::to_string_pretty(&version_json).unwrap(),
    )
    .await
    .unwrap();

    decompression_files(zip, decompression_tasks).await;

    Version::from_value(version_json).unwrap().id
}