use wasm_bindgen::prelude::*;
use wasm_bindgen::*;
use wasmparser::{Parser,Payload};
use anyhow::{Result};
use std::rc::{Rc};

mod dwarf;

use crate::dwarf::{ DwarfDebugInfo, VariableInfo, transform_dwarf };
use crate::dwarf::wasm_bindings::{ 
    WasmLineInfo, WasmValueVector, VariableVector
};


#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    // Note that this is using the `log` function imported above during
    // `bare_bones`
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct DwarfDebugSymbolContainer {
    debug_info: DwarfDebugInfo,
    code_base: usize,
    data_base: usize,
    data_ref: Rc<[u8]>
}

#[wasm_bindgen]
impl DwarfDebugSymbolContainer {

    pub fn new(data: &[u8]) -> Self {
        let data_rc: Rc<[u8]> = Rc::from(data);
        let base = calculate_code_base(data).ok().unwrap_or((0, 0));

        DwarfDebugSymbolContainer {
            code_base: base.0,
            data_base: base.1,
            debug_info: transform_dwarf(data_rc.clone()).unwrap(),
            data_ref: data_rc.clone()
        }
    }

    pub fn find_file_info_from_address(&self, instruction_offset: usize) -> Option<WasmLineInfo> {
        match self.debug_info.sourcemap.find_line_info(instruction_offset - self.code_base) {
            Some(x) => Some(WasmLineInfo::from_line_info(&x)),
            None => None
        }
    }

    pub fn find_address_from_file_info(&self, info: &WasmLineInfo) -> Option<usize> {
        let file_info = WasmLineInfo::into_line_info(info);
        match self.debug_info.sourcemap.find_address(&file_info) {
            Some(x) => Some(x + self.code_base),
            None => None
        }
    }

    pub fn variable_name_list(&self, instruction_offset: usize) -> Option<VariableVector> {
        match self.debug_info.subroutine.variable_name_list(instruction_offset - self.code_base) {
            Ok(x) => Some(VariableVector::from_vec(x)),
            Err(e) => { console_log!("{}", e); None }
        }
    }

    pub fn global_variable_name_list(&self, instruction: usize) -> Option<VariableVector> {

        let subroutine = match self.debug_info.subroutine.find_subroutine(instruction - self.code_base) 
        {
            Ok(x) => x,
            Err(e) => { console_log!("{}", e); return None; }
        };

        match self.debug_info.global_variables.variable_name_list(subroutine.unit_offset) {
            Ok(x) => Some(VariableVector::from_vec(x)),
            Err(e) => { console_log!("{}", e); None }
        }
    }

    pub fn get_variable_info(
        &self, 
        opts: String,
        locals: &WasmValueVector,
        globals: &WasmValueVector,
        stacks: &WasmValueVector,
        instruction_offset: usize) -> Option<VariableInfo> {

        match self.debug_info.subroutine.get_variable_info(&opts, locals, globals, stacks, instruction_offset - self.code_base) {
            Ok(Some(x)) => return Some(x),
            Ok(None) => {},
            Err(e) => { console_log!("{}", e)}
        };

        let subroutine = match self.debug_info.subroutine.find_subroutine(instruction_offset - self.code_base) 
        {
            Ok(x) => x,
            Err(e) => { console_log!("{}", e); return None; }
        };

        match self.debug_info.global_variables.get_variable_info(&opts, subroutine.unit_offset, self.data_base, globals) {
            Ok(x) => x,
            Err(e) => { console_log!("{}", e); None }
        }
    }
}

fn calculate_code_base(data: &[u8]) -> Result<(usize, usize)> {
    let parser = Parser::new(0);
    let mut code_section_offset = 0;
    let mut data_section_offset = 0;

    for payload in parser.parse_all(data) {
        match payload? {
            Payload::CodeSectionStart { range, .. } => {
                code_section_offset = range.start;
            },
            // Payload::DataSection(ref mut reader) => {
            //     let data = reader.read().expect("data");
               
            //     if let DataKind::Active { init_expr, .. } = data.kind {
            //         let mut init_expr_reader = init_expr.get_binary_reader();
            //         let op = init_expr_reader.read_operator().expect("op");
                    
            //         match op {
            //             wasmparser::Operator::I32Const { value } => {
            //                 data_section_offset = value as usize
            //             },
            //             _ => {}
            //         }
            //     }
            // },
            _ => continue
        }
    };
    Ok(
        (code_section_offset, data_section_offset)
    )
}
