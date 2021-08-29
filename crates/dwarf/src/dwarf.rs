use wasm_bindgen::prelude::*;
use object::{
    Object, ObjectSection
};
use gimli::{
    EndianRcSlice, LittleEndian, 
    Unit, UnitOffset, Reader,
    UnitSectionOffset, UnitHeader
};
use anyhow::{anyhow, Result};
use std::rc::{Rc};
use std::borrow::Borrow;

pub mod sourcemap;
pub mod subroutine;
pub mod variables;
pub mod wasm_bindings;

mod format;
mod utils;

use sourcemap::{ DwarfSourceMap, transform_debug_line };
use subroutine::{ DwarfSubroutineMap, transform_subprogram };
use format::{ format_object };
use utils::{ clone_string_attribute };

pub type DwarfReader = EndianRcSlice<LittleEndian>;
pub type DwarfReaderOffset = <DwarfReader as Reader>::Offset;
pub type Dwarf = gimli::Dwarf<DwarfReader>;

pub fn parse_dwarf(data: &[u8]) -> Result<Dwarf> {
    let object = object::File::parse(data.borrow())?;
    let endian = gimli::LittleEndian;

    // Load a section and return as `Cow<[u8]>`.
    let load_section = |id: gimli::SectionId| -> Result<Rc<[u8]>> {
        match object.section_by_name(id.name()) {
            Some(ref section) => Ok(Rc::from(section.data().unwrap_or(&[][..]))),
            None => Ok(Rc::from(&[][..])),
        }
    };

    // Load all of the sections.
    let dwarf_cow = gimli::Dwarf::load(&load_section)?;

    // Borrow a `Cow<[u8]>` to create an `EndianSlice`.
    let borrow_section = |section: &Rc<[u8]>| -> gimli::EndianRcSlice<gimli::LittleEndian> { 
        gimli::EndianRcSlice::new(section.clone(), endian) 
    };

    // Create `EndianSlice`s for all of the sections.
    Ok(dwarf_cow.borrow(&borrow_section))
}

pub struct DwarfDebugInfo {
    pub sourcemap: DwarfSourceMap,
    pub subroutine: DwarfSubroutineMap,
}

pub fn transform_dwarf(buffer: Rc<[u8]>) -> Result<DwarfDebugInfo> {
    let dwarf = parse_dwarf(buffer.borrow())?;
    let mut headers = dwarf.units();
    let mut sourcemaps = Vec::new();
    let mut subroutines = Vec::new();

    while let Some(header) = headers.next()? {
        let header_offset = header.offset();
        let unit = dwarf.unit(header)?;
        let mut entries = unit.entries();
        let root = match entries.next_dfs()? {
            Some((_, entry)) => entry,
            None => continue,
        };
        sourcemaps.push(transform_debug_line(
            &unit,
            root,
            &dwarf,
            &dwarf.debug_line,
        )?);
        subroutines.append(&mut transform_subprogram(&dwarf, &unit, header_offset)?);
    }
    Ok(DwarfDebugInfo {
        sourcemap: DwarfSourceMap::new(sourcemaps),
        subroutine: DwarfSubroutineMap {
            subroutines,
            buffer: buffer.clone(),
        },
    })
}

fn header_from_offset<R: gimli::Reader>(
    dwarf: &gimli::Dwarf<R>,
    offset: UnitSectionOffset<R::Offset>,
) -> Result<Option<UnitHeader<R>>> {
    let mut headers = dwarf.units();
    while let Some(header) = headers.next()? {
        if header.offset() == offset {
            return Ok(Some(header));
        } else {
            continue;
        }
    }
    return Ok(None);
}

fn unit_type_name<R: gimli::Reader>(
    dwarf: &gimli::Dwarf<R>,
    unit: &Unit<R>,
    type_offset: Option<R::Offset>,
) -> Result<String> {
    let type_offset = match type_offset {
        Some(offset) => offset,
        None => {
            return Ok("void".to_string());
        }
    };
    let mut tree = unit.entries_tree(Some(UnitOffset::<R::Offset>(type_offset)))?;
    let root = tree.root()?;
    if let Some(attr) = root.entry().attr_value(gimli::DW_AT_name)? {
        clone_string_attribute(dwarf, unit, attr)
    } else {
        Err(anyhow!(format!("failed to seek at {:?}", type_offset)))
    }
}

#[wasm_bindgen]
pub struct VariableInfo {
    pub address: usize,
    pub byte_size: usize,

    name: String,
    memory_slice: Vec<u8>,

    tag: gimli::DwTag,
    encoding: gimli::DwAte,
}

#[wasm_bindgen]
impl VariableInfo {
    pub fn set_memory_slice(&mut self, data: &[u8]) {
        self.memory_slice = data.to_vec();
    }

    pub fn print(&self) -> Option<String> {
        match format_object(self) {
            Ok(str) => { Some(str) },
            Err(_) => { None }
        }
    }
}
