use super::*;
use crate::VmContext;

pub fn host_abs(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let n = vm.pop() as i32;
    vm.push(if n < 0 { n.wrapping_neg() as u64 } else { n as u64 });
}

// ========== math.h Host Functions ==========

pub fn host_sin(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::sin(x).to_bits());
}

pub fn host_cos(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::cos(x).to_bits());
}

pub fn host_sqrt(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::sqrt(x).to_bits());
}

pub fn host_pow(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    let y = f64::from_bits(vm.pop());
    vm.push(libm::pow(x, y).to_bits());
}

pub fn host_atan(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::atan(x).to_bits());
}

pub fn host_log(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::log(x).to_bits());
}

pub fn host_exp(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::exp(x).to_bits());
}

pub fn host_tan(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::tan(x).to_bits());
}

pub fn host_log10(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::log10(x).to_bits());
}

pub fn host_fabs(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::fabs(x).to_bits());
}

pub fn host_ceil(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::ceil(x).to_bits());
}

pub fn host_floor(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::floor(x).to_bits());
}

pub fn host_round(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::round(x).to_bits());
}

pub fn host_fmod(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    let y = f64::from_bits(vm.pop());
    vm.push(libm::fmod(x, y).to_bits());
}

pub fn host_asin(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::asin(x).to_bits());
}

pub fn host_acos(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::acos(x).to_bits());
}

pub fn host_atan2(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let y = f64::from_bits(vm.pop());
    let x = f64::from_bits(vm.pop());
    vm.push(libm::atan2(y, x).to_bits());
}

pub fn host_sinh(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::sinh(x).to_bits());
}

pub fn host_cosh(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::cosh(x).to_bits());
}

pub fn host_tanh(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let x = f64::from_bits(vm.pop());
    vm.push(libm::tanh(x).to_bits());
}

pub fn host_llabs(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let n = vm.pop() as i64;
    vm.push(if n < 0 { n.wrapping_neg() as u64 } else { n as u64 });
}
