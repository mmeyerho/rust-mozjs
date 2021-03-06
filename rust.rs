#[doc = "Rust wrappers around the raw JS apis"];

import libc::types::os::arch::c95::size_t;

export rt;
export cx;
export jsobj;
export methods;

// ___________________________________________________________________________
// friendly Rustic API to runtimes

type rt = @rt_rsrc;

resource rt_rsrc(self: {ptr: *JSRuntime}) {
    JS_Finish(self.ptr)
}

fn rt() -> rt {
    @rt_rsrc({ptr: JS_Init(default_heapsize)})
}

impl methods for rt {
    fn cx() -> cx {
        @cx_rsrc({ptr: JS_NewContext(self.ptr, default_stacksize as size_t),
                  rt: self})
    }
}

// ___________________________________________________________________________
// contexts

type cx = @cx_rsrc;
resource cx_rsrc(self: {ptr: *JSContext, rt: rt}) {
    JS_DestroyContext(self.ptr);
}

impl methods for cx {
    fn rooted_obj(obj: *JSObject) -> jsobj {
        let jsobj = @jsobj_rsrc({cx: self, cxptr: self.ptr, ptr: obj});
        JS_AddObjectRoot(self.ptr, ptr::addr_of(jsobj.ptr));
        jsobj
    }

    fn set_default_options_and_version() {
        self.set_options(JSOPTION_VAROBJFIX | JSOPTION_METHODJIT);
        self.set_version(JSVERSION_LATEST);
    }

    fn set_options(v: jsuint) {
        JS_SetOptions(self.ptr, v);
    }

    fn set_version(v: i32) {
        JS_SetVersion(self.ptr, v);
    }

    fn set_logging_error_reporter() {
        JS_SetErrorReporter(self.ptr, reportError);
    }

    fn set_error_reporter(reportfn: *u8) {
        JS_SetErrorReporter(self.ptr, reportfn);
    }

    fn new_compartment(globclsfn: fn(name_pool) -> JSClass) -> result<compartment,()> {
        let np = name_pool();
        let globcls = @globclsfn(np);
        let globobj =
            JS_NewCompartmentAndGlobalObject(
                self.ptr,
                &*globcls as *JSClass,
                null());
        result(JS_InitStandardClasses(self.ptr, globobj)).chain { |_ok|
            ok(@{cx: self,
                 name_pool: np,
                 global_class: globcls,
                 mut global_funcs: [],
                 global_obj: self.rooted_obj(globobj)})
        }
    }

    fn evaluate_script(glob: jsobj, bytes: [u8], filename: str,
                       line_num: uint) -> result<(),()> {
        vec::as_buf(bytes) { |bytes_ptr|
            str::as_c_str(filename) { |filename_cstr|
                let bytes_ptr = bytes_ptr as *c_char;
                let v: jsval = 0_u64;
                #debug["Evaluating script from %s with bytes %?", filename, bytes];
                if JS_EvaluateScript(self.ptr, glob.ptr,
                                     bytes_ptr, bytes.len() as uintN,
                                     filename_cstr, line_num as uintN,
                                     ptr::addr_of(v)) == ERR {
                    #debug["...err!"];
                    err(())
                } else {
                    // we could return the script result but then we'd have
                    // to root it and so forth and, really, who cares?
                    #debug["...ok!"];
                    ok(())
                }
            }
        }
    }
}

crust fn reportError(_cx: *JSContext,
                     msg: *c_char,
                     report: *JSErrorReport) {
    unsafe {
        let fnptr = (*report).filename;
        let fname = if fnptr.is_not_null() {from_c_str(fnptr)} else {"none"};
        let lineno = (*report).lineno;
        let msg = from_c_str(msg);
        #error["Error at %s:%?: %s\n", fname, lineno, msg];
    }
}

// ___________________________________________________________________________
// compartment

type compartment = @{
    cx: cx,
    name_pool: name_pool,
    global_class: @JSClass,
    mut global_funcs: [@[JSFunctionSpec]],
    global_obj: jsobj
};

impl methods for compartment {
    fn define_functions(specfn: fn(name_pool) -> [JSFunctionSpec]) -> result<(),()> {
        let specvec = @specfn(self.name_pool);
        self.global_funcs += [specvec];
        vec::as_buf(*specvec) { |specs|
            result(JS_DefineFunctions(self.cx.ptr, self.global_obj.ptr, specs))
        }
    }
}

// ___________________________________________________________________________
// objects

type jsobj = @jsobj_rsrc;

resource jsobj_rsrc(self: {cx: cx, cxptr: *JSContext, ptr: *JSObject}) {
    JS_RemoveObjectRoot(self.cxptr, ptr::addr_of(self.ptr));
}

#[cfg(test)]
mod test {

    #[test]
    fn dummy() {
        let rt = rt();
        let cx = rt.cx();
        cx.set_default_options_and_version();
        cx.set_logging_error_reporter();
        cx.new_compartment(global::global_class).chain { |comp|
            comp.define_functions(global::debug_fns);

            let bytes = str::bytes("debug(22);");
            cx.evaluate_script(comp.global_obj, bytes, "test", 1u)
        };
    }

}
