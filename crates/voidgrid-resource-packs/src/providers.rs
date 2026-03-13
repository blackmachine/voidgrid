use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use anyhow::{Context, Result};
use zip::ZipArchive;
use voidgrid::resource_pack::ResourceProvider;

pub struct DirProvider {
    base_path: PathBuf,
}

impl DirProvider {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }
}

impl ResourceProvider for DirProvider {
    fn read_bytes(&mut self, path: &str) -> Result<Vec<u8>> {
        let full_path = self.base_path.join(path);
        std::fs::read(&full_path).with_context(|| format!("Failed to read bytes from file: {:?}", full_path))
    }

    fn read_string(&mut self, path: &str) -> Result<String> {
        let full_path = self.base_path.join(path);
        let content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read string from file: {:?}", full_path))?;
        
        // РЈРґР°Р»СЏРµРј РЅРµРІРёРґРёРјС‹Р№ BOM, РєРѕС‚РѕСЂС‹Р№ РґРѕР±Р°РІР»СЏСЋС‚ СЂРµРґР°РєС‚РѕСЂС‹
        Ok(content.strip_prefix('\u{FEFF}').unwrap_or(&content).to_string())
    }
}

pub struct ZipProvider {
    archive: ZipArchive<File>,
}

impl ZipProvider {
    pub fn new(file: File) -> Result<Self> {
        Ok(Self {
            archive: ZipArchive::new(file)?,
        })
    }
}

impl ResourceProvider for ZipProvider {
    fn read_bytes(&mut self, path: &str) -> Result<Vec<u8>> {
        let normalized_path = path.replace('\\', "/");
        let mut file = self.archive.by_name(&normalized_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    fn read_string(&mut self, path: &str) -> Result<String> {
        let normalized_path = path.replace('\\', "/");
        let mut file = self.archive.by_name(&normalized_path)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        
        // РЈРґР°Р»СЏРµРј РЅРµРІРёРґРёРјС‹Р№ BOM, РєРѕС‚РѕСЂС‹Р№ РґРѕР±Р°РІР»СЏСЋС‚ СЂРµРґР°РєС‚РѕСЂС‹
        Ok(buffer.strip_prefix('\u{FEFF}').unwrap_or(&buffer).to_string())
    }
}
