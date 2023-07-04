use std::{
    fmt::Display,
    format,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
/// The Minecraft folder structure. All method will return the path related to a minecraft root like .minecraft.
pub struct MinecraftLocation {
    pub root: PathBuf,
    pub libraries: PathBuf,
    pub assets: PathBuf,
    pub resourcepacks: PathBuf,
    pub mods: PathBuf,
    pub logs: PathBuf,
    pub latest_log: PathBuf,
    pub saves: PathBuf,
    pub versions: PathBuf,
    pub options: PathBuf,
    pub screenshots: PathBuf,
}

impl MinecraftLocation {
    pub fn new(root: &str) -> MinecraftLocation {
        let path = Path::new(root);
        MinecraftLocation {
            root: path.to_path_buf(),
            assets: path.join("assets"),
            libraries: path.join("libraries"),
            resourcepacks: path.join("resourcepacks"),
            mods: path.join("mods"),
            logs: path.join("logs"),
            latest_log: path.join("logs").join("latest.log"),
            saves: path.join("resourcepacks"),
            versions: path.join("versions"),
            options: path.join("options.txt"),
            screenshots: path.join("screenshots"),
        }
    }

    pub fn get_natives_root(&self, version: &str) -> PathBuf {
        PathBuf::from(version).join(format!("{version}-natives.jar"))
    }

    pub fn get_version_root<P: AsRef<Path>>(&self, version: P) -> PathBuf {
        PathBuf::from(self.versions.clone()).join(version)
    }

    pub fn get_version_json<P: AsRef<Path> + Display>(&self, version: P) -> PathBuf {
        PathBuf::from(self.get_version_root(&version)).join(format!("{version}.json"))
    }

    pub fn get_version_jar<P: AsRef<Path> + Display>(&self, version: P, r#type: Option<&str>) -> PathBuf {
        if r#type == Some("client") || r#type.is_none() {
            self.get_version_root(&version)
                .join(format!("{version}.jar"))
        } else {
            self.get_version_root(&version)
                .join(format!("{version}-{}.jar", r#type.unwrap()))
        }
    }

    pub fn get_version_all<P: AsRef<Path> + Display>(&self, version: P) -> Vec<PathBuf> {
        vec![
            self.versions.join(&version),
            self.versions.join(&version).join(format!("{version}.json")),
            self.versions.join(&version).join(format!("{version}.jar")),
        ]
    }

    pub fn get_resource_pack<P: AsRef<Path>>(&self, file_name: P) -> PathBuf {
        self.resourcepacks.join(file_name)
    }

    pub fn get_mod<P: AsRef<Path>>(&self, file_name: P) -> PathBuf {
        self.mods.join(file_name)
    }

    pub fn get_log<P: AsRef<Path>>(&self, file_name: P) -> PathBuf {
        self.logs.join(file_name)
    }

    pub fn get_library_by_path<P: AsRef<Path>>(&self, library_path: P) -> PathBuf {
        self.libraries.join(library_path)
    }
}

pub fn get_path(path: &PathBuf) -> String {
    match path.to_str() {
        None => panic!("New path is noe a valid UTF-8 sequence!"),
        Some(s) => String::from(s),
    }
}

#[test]
fn test() {
    let a = MinecraftLocation::new("/home/CD-DVD/test");
    println!("{:#?}", a);
}