#![allow(dead_code)]

use crate::efi::*;
use core::ffi::c_void;

#[repr(C)]
struct ServiceBinding {
    create_child: usize,
    destroy_child: usize,
}

#[repr(C)]
struct HttpV4 {
    use_default: u8,
    local_addr: [u8; 4],
    local_subnet: [u8; 4],
    local_port: u16,
}

#[repr(C)]
struct HttpConfig {
    version: u32,
    timeout: u32,
    local_is_ipv6: u8,
    access: *mut HttpV4,
}

#[repr(C)]
struct HttpRequestData {
    method: u32,
    url: *mut u16,
}

#[repr(C)]
struct HttpResponseData {
    status: u32,
}

#[repr(C)]
struct HttpHeader {
    name: *mut u8,
    value: *mut u8,
}

#[repr(C)]
struct HttpMessage {
    data: *mut c_void,
    header_count: usize,
    headers: *mut HttpHeader,
    body_length: usize,
    body: *mut c_void,
}

#[repr(C)]
struct HttpToken {
    event: *mut c_void,
    status: Status,
    message: *mut HttpMessage,
}

#[repr(C)]
struct HttpProtocol {
    get_mode_data: usize,
    configure: usize,
    request: usize,
    cancel: usize,
    response: usize,
    poll: usize,
}

pub const ERR_NO_SERVICE: u32 = 1;
pub const ERR_CONNECT: u32 = 2;
pub const ERR_REQUEST: u32 = 3;
pub const ERR_RESPONSE: u32 = 4;
pub const ERR_EMPTY: u32 = 5;
pub const ERR_TOO_BIG: u32 = 6;
pub const ERR_REDIRECT: u32 = 7;

unsafe fn wait_event(ev: *mut c_void, http: *mut HttpProtocol, ms: u32) -> bool {
    let poll: extern "efiapi" fn(*mut HttpProtocol) -> Status = core::mem::transmute((*http).poll);
    let mut i = 0u32;
    while i < ms {
        if status_ok(bs_check_event(ev)) {
            return true;
        }
        let _ = poll(http);
        bs_stall(1_000);
        i += 1;
    }
    false
}

fn lower(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { c + 32 } else { c }
}

unsafe fn copy_header(msg: *mut HttpMessage, name: &[u8], out: &mut [u8]) -> usize {
    if (*msg).headers.is_null() { return 0; }
    let n = if (*msg).header_count > 64 { 64 } else { (*msg).header_count };
    let h = (*msg).headers;
    let mut i = 0;
    while i < n {
        let hn = (*h.add(i)).name;
        let hv = (*h.add(i)).value;
        if !hn.is_null() && !hv.is_null() {
            let mut k = 0usize;
            loop {
                let c = *hn.add(k);
                if k >= name.len() || c == 0 { break; }
                if lower(c) != lower(name[k]) { break; }
                k += 1;
            }
            let end_ok = k == name.len() && *hn.add(k) == 0;
            if end_ok {
                let mut l = 0usize;
                while l + 1 < out.len() && *hv.add(l) != 0 {
                    out[l] = *hv.add(l);
                    l += 1;
                }
                return l;
            }
        }
        i += 1;
    }
    0
}

unsafe fn free_headers(msg: *mut HttpMessage) {
    if (*msg).headers.is_null() {
        return;
    }
    let n = if (*msg).header_count > 64 { 64 } else { (*msg).header_count };
    let h = (*msg).headers;
    let mut i = 0;
    while i < n {
        let hn = (*h.add(i)).name;
        let hv = (*h.add(i)).value;
        if !hn.is_null() { bs_free_pool(hn); }
        if !hv.is_null() { bs_free_pool(hv); }
        i += 1;
    }
    bs_free_pool(h as *mut u8);
}

unsafe fn http_get_once(http: *mut HttpProtocol, url: &str, out: &mut [u8], redirect: &mut [u8]) -> Result<usize, u32> {
    let mut url16 = [0u16; 320];
    let ul = to_utf16(url, &mut url16);
    if ul + 1 >= url16.len() {
        return Err(ERR_TOO_BIG);
    }
    let mut node = HttpV4 {
        use_default: 1,
        local_addr: [0; 4],
        local_subnet: [0; 4],
        local_port: 0,
    };
    let mut cfg = HttpConfig {
        version: 1,
        timeout: 8000,
        local_is_ipv6: 0,
        access: &mut node as *mut HttpV4,
    };
    let configure: extern "efiapi" fn(*mut HttpProtocol, *mut HttpConfig) -> Status =
        core::mem::transmute((*http).configure);
    let st = configure(http, &mut cfg);
    if !status_ok(st) && st != 0x8000_0000_0000_0014 {
        return Err(ERR_CONNECT);
    }

    let mut ev_req: *mut c_void = core::ptr::null_mut();
    if !status_ok(bs_create_event(0, core::ptr::null_mut(), core::ptr::null_mut(), &mut ev_req)) {
        return Err(ERR_REQUEST);
    }
    let mut name1 = *b"User-Agent\0";
    let mut val1 = *b"HYWOS-Update/1.1.3\0";
    let mut name2 = *b"Accept\0";
    let mut val2 = *b"application/vnd.github+json\0";
    let mut headers = [
        HttpHeader { name: name1.as_mut_ptr(), value: val1.as_mut_ptr() },
        HttpHeader { name: name2.as_mut_ptr(), value: val2.as_mut_ptr() },
    ];
    let mut req = HttpRequestData { method: 0, url: url16.as_mut_ptr() };
    let mut msg = HttpMessage {
        data: &mut req as *mut HttpRequestData as *mut c_void,
        header_count: 2,
        headers: headers.as_mut_ptr(),
        body_length: 0,
        body: core::ptr::null_mut(),
    };
    let mut tok = HttpToken {
        event: ev_req,
        status: 0,
        message: &mut msg as *mut HttpMessage,
    };
    let request: extern "efiapi" fn(*mut HttpProtocol, *mut HttpToken) -> Status =
        core::mem::transmute((*http).request);
    if !status_ok(request(http, &mut tok)) {
        bs_close_event(ev_req);
        return Err(ERR_REQUEST);
    }
    if !wait_event(ev_req, http, 8_000) || !status_ok(tok.status) {
        bs_close_event(ev_req);
        return Err(ERR_REQUEST);
    }
    bs_close_event(ev_req);

    let mut ev_rsp: *mut c_void = core::ptr::null_mut();
    if !status_ok(bs_create_event(0, core::ptr::null_mut(), core::ptr::null_mut(), &mut ev_rsp)) {
        return Err(ERR_RESPONSE);
    }
    let mut resp = HttpResponseData { status: 0 };
    let mut msg2 = HttpMessage {
        data: &mut resp as *mut HttpResponseData as *mut c_void,
        header_count: 0,
        headers: core::ptr::null_mut(),
        body_length: 0,
        body: core::ptr::null_mut(),
    };
    let mut tok2 = HttpToken {
        event: ev_rsp,
        status: 0,
        message: &mut msg2 as *mut HttpMessage,
    };
    let response: extern "efiapi" fn(*mut HttpProtocol, *mut HttpToken) -> Status =
        core::mem::transmute((*http).response);
    if !status_ok(response(http, &mut tok2)) {
        bs_close_event(ev_rsp);
        return Err(ERR_RESPONSE);
    }
    if !wait_event(ev_rsp, http, 12_000) || !status_ok(tok2.status) {
        free_headers(&mut msg2);
        if !msg2.body.is_null() { bs_free_pool(msg2.body as *mut u8); }
        bs_close_event(ev_rsp);
        return Err(ERR_RESPONSE);
    }
    bs_close_event(ev_rsp);
    let loc_len = copy_header(&mut msg2, b"Location", redirect);
    let body_len = msg2.body_length;
    if loc_len > 0 && body_len == 0 {
        free_headers(&mut msg2);
        if !msg2.body.is_null() { bs_free_pool(msg2.body as *mut u8); }
        return Err(ERR_REDIRECT);
    }
    if body_len == 0 || msg2.body.is_null() {
        free_headers(&mut msg2);
        if !msg2.body.is_null() { bs_free_pool(msg2.body as *mut u8); }
        return Err(ERR_EMPTY);
    }
    if body_len > out.len() {
        free_headers(&mut msg2);
        bs_free_pool(msg2.body as *mut u8);
        return Err(ERR_TOO_BIG);
    }
    let src = core::slice::from_raw_parts(msg2.body as *const u8, body_len);
    out[..body_len].copy_from_slice(src);
    free_headers(&mut msg2);
    bs_free_pool(msg2.body as *mut u8);
    Ok(body_len)
}

pub unsafe fn get(url: &str, out: &mut [u8]) -> Result<usize, u32> {
    bs_connect_all_network();
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if !status_ok(bs_locate_handle_buffer(&GUID_HTTP_SERVICE_BINDING, &mut n, &mut buf)) || buf.is_null() {
        return Err(ERR_NO_SERVICE);
    }
    let mut last = ERR_NO_SERVICE;
    let mut i = 0;
    while i < n {
        let mut sb: *mut ServiceBinding = core::ptr::null_mut();
        if status_ok(bs_handle_protocol(
            *buf.add(i),
            &GUID_HTTP_SERVICE_BINDING,
            &mut sb as *mut *mut ServiceBinding as *mut *mut c_void,
        )) && !sb.is_null()
        {
            let create: extern "efiapi" fn(*mut ServiceBinding, *mut Handle) -> Status =
                core::mem::transmute((*sb).create_child);
            let destroy: extern "efiapi" fn(*mut ServiceBinding, Handle) -> Status =
                core::mem::transmute((*sb).destroy_child);
            let mut child: Handle = core::ptr::null_mut();
            if status_ok(create(sb, &mut child)) && !child.is_null() {
                let mut http: *mut HttpProtocol = core::ptr::null_mut();
                if status_ok(bs_handle_protocol(
                    child,
                    &GUID_HTTP_PROTOCOL,
                    &mut http as *mut *mut HttpProtocol as *mut *mut c_void,
                )) && !http.is_null()
                {
                    let mut cur = [0u8; 512];
                    let mut cl = 0usize;
                    for c in url.bytes() {
                        if cl < 510 { cur[cl] = c; cl += 1; }
                    }
                    let mut depth = 0;
                    let result: Result<usize, u32>;
                    loop {
                        let cur_str = core::str::from_utf8_unchecked(&cur[..cl]);
                        let mut redirect = [0u8; 512];
                        match http_get_once(http, cur_str, out, &mut redirect) {
                            Err(ERR_REDIRECT) if depth < 3 => {
                                depth += 1;
                                cl = 0;
                                let mut k = 0;
                                while k < redirect.len() && redirect[k] != 0 && cl + 1 < cur.len() { cur[cl] = redirect[k]; cl += 1; k += 1; }
                                if cl == 0 { result = Err(ERR_REDIRECT); break; }
                            }
                            r => { result = r; break; }
                        }
                    }
                    let _ = destroy(sb, child);
                    if result.is_ok() {
                        bs_free_pool(buf as *mut u8);
                        return result;
                    }
                    last = match result { Err(e) => e, Ok(_) => 0 };
                } else {
                    let _ = destroy(sb, child);
                }
            }
        }
        i += 1;
    }
    bs_free_pool(buf as *mut u8);
    Err(last)
}