/**
 * Returns the normalized 3D bar target derived from real suite, kernel, and sample progress.
 * @param {number} stage_index
 * @param {number} stage_count
 * @param {number} kernel_index
 * @param {number} kernel_count
 * @param {number} sample_count
 * @param {number} sample_total
 * @returns {number}
 */
export function bar_progress_fraction(stage_index, stage_count, kernel_index, kernel_count, sample_count, sample_total) {
    const ret = wasm.bar_progress_fraction(stage_index, stage_count, kernel_index, kernel_count, sample_count, sample_total);
    return ret;
}

/**
 * Cancels restoration of an in-flight WebGPU compute suite after a page watchdog expires.
 */
export function cancel_gpu_compute() {
    wasm.cancel_gpu_compute();
}

/**
 * Returns the pinned faer wasm compile verdict represented by this loaded module.
 * @returns {string}
 */
export function faer_wasm_verdict_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.faer_wasm_verdict_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Returns the scalar contraction and baked-reference transcendental fingerprint as JSON.
 * @returns {string}
 */
export function float_probe_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.float_probe_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Returns the stable compute-only WebGPU kernel inventory as JSON.
 * @returns {string}
 */
export function gpu_compute_inventory_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.gpu_compute_inventory_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Returns whether the initialized WebGPU compute device is still usable, including its device
 * loss reason when it is not.
 * @returns {string}
 */
export function gpu_compute_status_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.gpu_compute_status_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Requests a compute-only WebGPU adapter and device, compiles the fixed pipelines, and returns
 * adapter facts as JSON.
 *
 * # Errors
 *
 * Returns a JavaScript error when WebGPU is absent, the browser refuses an adapter or device, or
 * the fixed shader and pipeline warmup fails validation.
 * @returns {Promise<string>}
 */
export function initialize_gpu_compute_json() {
    const ret = wasm.initialize_gpu_compute_json();
    return ret;
}

/**
 * Initializes the surface-backed WebGPU progress bar and returns adapter and surface facts as
 * JSON. The progress renderer owns a device separate from the compute suite.
 *
 * # Errors
 *
 * Returns a JavaScript error when WebGPU is absent, no canvas-compatible adapter is available, or
 * surface or pipeline initialization fails.
 * @param {HTMLCanvasElement} canvas
 * @returns {Promise<string>}
 */
export function initialize_render_bar_json(canvas) {
    const ret = wasm.initialize_render_bar_json(canvas);
    return ret;
}

/**
 * Returns the stable timed-kernel inventory as JSON.
 * @returns {string}
 */
export function kernel_inventory_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.kernel_inventory_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Pumps the opt-in submission and returns its current status as JSON.
 * @returns {string}
 */
export function poll_submission_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.poll_submission_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Presents one progress frame and returns the renderer's cumulative presented-frame count.
 *
 * # Errors
 *
 * Returns a JavaScript error when the renderer is unavailable, its device was lost, or surface
 * acquisition or presentation fails.
 * @param {number} progress
 * @returns {number}
 */
export function render_bar_frame(progress) {
    const ret = wasm.render_bar_frame(progress);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return ret[0] >>> 0;
}

/**
 * Drops the current progress surface and invalidates any pending renderer initialization.
 */
export function reset_render_bar() {
    wasm.reset_render_bar();
}

/**
 * Resets per-run kernel buffers and any previous submission connection.
 */
export function reset_run_state() {
    wasm.reset_run_state();
}

/**
 * Runs one adaptively repeated compute-only WebGPU workload and returns timing and validation
 * facts as JSON.
 *
 * # Errors
 *
 * Returns a JavaScript error for an unknown kernel, invalid repeat count, device loss, mapping
 * failure, non-finite output, or output that differs from the fixed expected values.
 * @param {string} kernel_id
 * @param {number} repeat_count
 * @returns {Promise<string>}
 */
export function run_gpu_compute_json(kernel_id, repeat_count) {
    const ptr0 = passStringToWasm0(kernel_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.run_gpu_compute_json(ptr0, len0, repeat_count);
    return ret;
}

/**
 * Runs one short arithmetic chunk for the page's controlled 50 ms main-thread burst.
 * @returns {number}
 */
export function run_jank_chunk() {
    const ret = wasm.run_jank_chunk();
    return ret;
}

/**
 * Runs one preallocated fixed workload and returns its opaque checksum.
 *
 * # Errors
 *
 * Returns a JavaScript error when the supplied kernel identifier is unknown.
 * @param {string} kernel_id
 * @returns {number}
 */
export function run_kernel(kernel_id) {
    const ptr0 = passStringToWasm0(kernel_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.run_kernel(ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return ret[0];
}

/**
 * Runs repeated base invocations inside one wasm call for coarse-timer measurement.
 *
 * # Errors
 *
 * Returns a JavaScript error for zero repeats or an unknown kernel identifier.
 * @param {string} kernel_id
 * @param {number} repeat_count
 * @returns {number}
 */
export function run_kernel_repeated(kernel_id, repeat_count) {
    const ptr0 = passStringToWasm0(kernel_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.run_kernel_repeated(ptr0, len0, repeat_count);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return ret[0];
}

/**
 * Validates and starts the sole opt-in network submission through the canonical outer protocol.
 *
 * # Errors
 *
 * Returns a JavaScript error for invalid report JSON, an oversized report, an invalid URL, or an
 * immediate browser WebSocket failure.
 * @param {string} server_url
 * @param {string} report_json
 */
export function start_submission(server_url, report_json) {
    const ptr0 = passStringToWasm0(server_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(report_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.start_submission(ptr0, len0, ptr1, len1);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * Returns the deterministic, measurement-derived verdict presentation as JSON.
 *
 * # Errors
 *
 * Returns a JavaScript error when the supplied document is not a valid schema-1 report.
 * @param {string} report_json
 * @returns {string}
 */
export function verdict_json(report_json) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(report_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.verdict_json(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Initializes the wasm module without starting a renderer or network connection.
 */
export function wasm_init() {
    wasm.wasm_init();
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Window_412fe051c1aa1519: function(arg0) {
            const ret = arg0.Window;
            return ret;
        },
        __wbg_WorkerGlobalScope_349300f9b277afe1: function(arg0) {
            const ret = arg0.WorkerGlobalScope;
            return ret;
        },
        __wbg___wbindgen_debug_string_a57024b9c6e4a48b: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_5e4570eb24ffa122: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_7d13f41e1a2d5140: function(arg0) {
            const ret = arg0 === null;
            return ret;
        },
        __wbg___wbindgen_is_object_a2790eb24c211ea0: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_undefined_6cff064c44e0d823: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_number_get_136b9679cab35cfb: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_d154f1e671052120: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_bb96b2010945f0bc: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_be22cc64ae6946a0: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_beginComputePass_097033d61ef8af0f: function(arg0, arg1) {
            const ret = arg0.beginComputePass(arg1);
            return ret;
        },
        __wbg_beginRenderPass_34a015f8265125ac: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.beginRenderPass(arg1);
            return ret;
        }, arguments); },
        __wbg_buffer_78291c0e094ccf99: function(arg0) {
            const ret = arg0.buffer;
            return ret;
        },
        __wbg_bufferedAmount_8439cda0fbab40f3: function(arg0) {
            const ret = arg0.bufferedAmount;
            return ret;
        },
        __wbg_call_1c5886ab9c57d1c7: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_call_35dba3c747ad7521: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_clearInterval_db08de379bb142dc: function(arg0, arg1) {
            arg0.clearInterval(arg1);
        },
        __wbg_code_e2719108dd8e1fec: function(arg0) {
            const ret = arg0.code;
            return ret;
        },
        __wbg_configure_ce7dc0ea629bbc55: function() { return handleError(function (arg0, arg1) {
            arg0.configure(arg1);
        }, arguments); },
        __wbg_copyBufferToBuffer_99ba10ae51f20b8a: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.copyBufferToBuffer(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_createBindGroupLayout_1d37ac0dabfbed28: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBindGroupLayout(arg1);
            return ret;
        }, arguments); },
        __wbg_createBindGroup_3bccbd7517f0708e: function(arg0, arg1) {
            const ret = arg0.createBindGroup(arg1);
            return ret;
        },
        __wbg_createBuffer_24b346170c9f54c8: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBuffer(arg1);
            return ret;
        }, arguments); },
        __wbg_createCommandEncoder_48a406baaa084912: function(arg0, arg1) {
            const ret = arg0.createCommandEncoder(arg1);
            return ret;
        },
        __wbg_createComputePipeline_4efb4ca205a4b557: function(arg0, arg1) {
            const ret = arg0.createComputePipeline(arg1);
            return ret;
        },
        __wbg_createPipelineLayout_f668b6fbdf877ab3: function(arg0, arg1) {
            const ret = arg0.createPipelineLayout(arg1);
            return ret;
        },
        __wbg_createQuerySet_a6bdf70e581bbde3: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createQuerySet(arg1);
            return ret;
        }, arguments); },
        __wbg_createRenderPipeline_def9fb5cd54c36d5: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createRenderPipeline(arg1);
            return ret;
        }, arguments); },
        __wbg_createShaderModule_1b0812f3a4503221: function(arg0, arg1) {
            const ret = arg0.createShaderModule(arg1);
            return ret;
        },
        __wbg_createTexture_77337549db437b45: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createTexture(arg1);
            return ret;
        }, arguments); },
        __wbg_createView_13bc5cdadcefa9ec: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createView(arg1);
            return ret;
        }, arguments); },
        __wbg_data_57d8ce4eb5f0a433: function(arg0) {
            const ret = arg0.data;
            return ret;
        },
        __wbg_dispatchWorkgroups_1b750cb68e2eb693: function(arg0, arg1, arg2, arg3) {
            arg0.dispatchWorkgroups(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0);
        },
        __wbg_document_ac38448dbfd31a57: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_draw_754f5b2022d90fd7: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.draw(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_end_1300dc816a60c7ab: function(arg0) {
            arg0.end();
        },
        __wbg_end_fd65a01a19361ec7: function(arg0) {
            arg0.end();
        },
        __wbg_features_efadd23951712b29: function(arg0) {
            const ret = arg0.features;
            return ret;
        },
        __wbg_finish_2440fb64e53f7d5a: function(arg0, arg1) {
            const ret = arg0.finish(arg1);
            return ret;
        },
        __wbg_finish_4b40810f0b577bc2: function(arg0) {
            const ret = arg0.finish();
            return ret;
        },
        __wbg_getContext_71c33f14b63da593: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_c5236e0057b35024: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getCurrentTexture_3ac9a0eb8d7eccf7: function() { return handleError(function (arg0) {
            const ret = arg0.getCurrentTexture();
            return ret;
        }, arguments); },
        __wbg_getMappedRange_55878eb97535ca19: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getMappedRange(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_getPreferredCanvasFormat_1af5deca596e85de: function(arg0) {
            const ret = arg0.getPreferredCanvasFormat();
            return (__wbindgen_enum_GpuTextureFormat.indexOf(ret) + 1 || 96) - 1;
        },
        __wbg_get_36debceb6d43d7a1: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_971a0c45d172643f: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_gpu_bafbc1407fe850fb: function(arg0) {
            const ret = arg0.gpu;
            return ret;
        },
        __wbg_has_dc80aa6186153231: function(arg0, arg1, arg2) {
            const ret = arg0.has(getStringFromWasm0(arg1, arg2));
            return ret;
        },
        __wbg_height_e56f6fb197710e09: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_instanceof_ArrayBuffer_993d02d2d254cad1: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuAdapter_aff4b0f95a6c1c3e: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUAdapter;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuCanvasContext_dc8dc7061b962990: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUCanvasContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuDeviceLostInfo_8a1670921dcc0ec6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUDeviceLostInfo;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuOutOfMemoryError_c9077369f40e8df6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUOutOfMemoryError;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuValidationError_6f92e479d86616a6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUValidationError;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Object_80ad464782e2bd73: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Object;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_5625ff9937037a38: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_label_4b6427d9045e3926: function(arg0, arg1) {
            const ret = arg1.label;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_length_36bd29c6848c2144: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_limits_805a5a24eacdef89: function(arg0) {
            const ret = arg0.limits;
            return ret;
        },
        __wbg_lost_5450e6b9e280ed71: function(arg0) {
            const ret = arg0.lost;
            return ret;
        },
        __wbg_mapAsync_f7fe2e4825742580: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.mapAsync(arg1 >>> 0, arg2, arg3);
            return ret;
        },
        __wbg_maxBindGroups_aeb19ade452446c6: function(arg0) {
            const ret = arg0.maxBindGroups;
            return ret;
        },
        __wbg_maxBindingsPerBindGroup_56b9cd783b976459: function(arg0) {
            const ret = arg0.maxBindingsPerBindGroup;
            return ret;
        },
        __wbg_maxBufferSize_b9cfa105ccd49524: function(arg0) {
            const ret = arg0.maxBufferSize;
            return ret;
        },
        __wbg_maxColorAttachmentBytesPerSample_3853759407ab3c40: function(arg0) {
            const ret = arg0.maxColorAttachmentBytesPerSample;
            return ret;
        },
        __wbg_maxColorAttachments_a9a3c5bc728fb56f: function(arg0) {
            const ret = arg0.maxColorAttachments;
            return ret;
        },
        __wbg_maxComputeInvocationsPerWorkgroup_732f87215035d9e5: function(arg0) {
            const ret = arg0.maxComputeInvocationsPerWorkgroup;
            return ret;
        },
        __wbg_maxComputeWorkgroupSizeX_4f1d6552edeba82a: function(arg0) {
            const ret = arg0.maxComputeWorkgroupSizeX;
            return ret;
        },
        __wbg_maxComputeWorkgroupSizeY_170c377843fcdda9: function(arg0) {
            const ret = arg0.maxComputeWorkgroupSizeY;
            return ret;
        },
        __wbg_maxComputeWorkgroupSizeZ_78bd13bc9226bc99: function(arg0) {
            const ret = arg0.maxComputeWorkgroupSizeZ;
            return ret;
        },
        __wbg_maxComputeWorkgroupStorageSize_0a6873ffe86d432d: function(arg0) {
            const ret = arg0.maxComputeWorkgroupStorageSize;
            return ret;
        },
        __wbg_maxComputeWorkgroupsPerDimension_7f64fa252d98d9e0: function(arg0) {
            const ret = arg0.maxComputeWorkgroupsPerDimension;
            return ret;
        },
        __wbg_maxDynamicStorageBuffersPerPipelineLayout_548c4d8427692343: function(arg0) {
            const ret = arg0.maxDynamicStorageBuffersPerPipelineLayout;
            return ret;
        },
        __wbg_maxDynamicUniformBuffersPerPipelineLayout_575f81e5a619fda4: function(arg0) {
            const ret = arg0.maxDynamicUniformBuffersPerPipelineLayout;
            return ret;
        },
        __wbg_maxSampledTexturesPerShaderStage_4b0d0d0deb7f9173: function(arg0) {
            const ret = arg0.maxSampledTexturesPerShaderStage;
            return ret;
        },
        __wbg_maxSamplersPerShaderStage_122e1c314b7d5f0e: function(arg0) {
            const ret = arg0.maxSamplersPerShaderStage;
            return ret;
        },
        __wbg_maxStorageBufferBindingSize_69368e8c4a720d65: function(arg0) {
            const ret = arg0.maxStorageBufferBindingSize;
            return ret;
        },
        __wbg_maxStorageBuffersPerShaderStage_483da9a48e09b2cd: function(arg0) {
            const ret = arg0.maxStorageBuffersPerShaderStage;
            return ret;
        },
        __wbg_maxStorageTexturesPerShaderStage_825095cb824c2a90: function(arg0) {
            const ret = arg0.maxStorageTexturesPerShaderStage;
            return ret;
        },
        __wbg_maxTextureArrayLayers_311d9cd973092ad3: function(arg0) {
            const ret = arg0.maxTextureArrayLayers;
            return ret;
        },
        __wbg_maxTextureDimension1D_f14696527b4dd4c9: function(arg0) {
            const ret = arg0.maxTextureDimension1D;
            return ret;
        },
        __wbg_maxTextureDimension2D_8a888981a9a496a3: function(arg0) {
            const ret = arg0.maxTextureDimension2D;
            return ret;
        },
        __wbg_maxTextureDimension3D_af7a8c47b3a93760: function(arg0) {
            const ret = arg0.maxTextureDimension3D;
            return ret;
        },
        __wbg_maxUniformBufferBindingSize_5cab04d98886e7d3: function(arg0) {
            const ret = arg0.maxUniformBufferBindingSize;
            return ret;
        },
        __wbg_maxUniformBuffersPerShaderStage_14f4345c06c80500: function(arg0) {
            const ret = arg0.maxUniformBuffersPerShaderStage;
            return ret;
        },
        __wbg_maxVertexAttributes_73ce901689262af1: function(arg0) {
            const ret = arg0.maxVertexAttributes;
            return ret;
        },
        __wbg_maxVertexBufferArrayStride_4cb22981054e6df0: function(arg0) {
            const ret = arg0.maxVertexBufferArrayStride;
            return ret;
        },
        __wbg_maxVertexBuffers_a6df2bc183ca7af0: function(arg0) {
            const ret = arg0.maxVertexBuffers;
            return ret;
        },
        __wbg_message_25bb811d1d056a2d: function(arg0, arg1) {
            const ret = arg1.message;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_message_e2fb350ad8cfe709: function(arg0, arg1) {
            const ret = arg1.message;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_minStorageBufferOffsetAlignment_2039bfddd5b42bd9: function(arg0) {
            const ret = arg0.minStorageBufferOffsetAlignment;
            return ret;
        },
        __wbg_minUniformBufferOffsetAlignment_4366c5e24c3a2e2f: function(arg0) {
            const ret = arg0.minUniformBufferOffsetAlignment;
            return ret;
        },
        __wbg_navigator_6cfdd5fa246d910f: function(arg0) {
            const ret = arg0.navigator;
            return ret;
        },
        __wbg_navigator_e5c345298a9609cd: function(arg0) {
            const ret = arg0.navigator;
            return ret;
        },
        __wbg_new_116be93542d39019: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_20a7c62e9b30cbf7: function() { return handleError(function (arg0, arg1) {
            const ret = new WebSocket(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_77cc4f4f472aeb81: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_ebe3e0f6837f0879: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_from_slice_3eea173078478cfe: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_typed_cceaf62d8d95e9f2: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___js_sys_bc21f7f944e8537f___Function_fn_wasm_bindgen_622e9445b6bb02a6___JsValue_____wasm_bindgen_622e9445b6bb02a6___sys__Undefined___js_sys_bc21f7f944e8537f___Function_fn_wasm_bindgen_622e9445b6bb02a6___JsValue_____wasm_bindgen_622e9445b6bb02a6___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_new_with_byte_offset_and_length_ff6e927f8d72f0c3: function(arg0, arg1, arg2) {
            const ret = new Uint8Array(arg0, arg1 >>> 0, arg2 >>> 0);
            return ret;
        },
        __wbg_now_8b265300afd5f2b9: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_popErrorScope_79543cb048170397: function(arg0) {
            const ret = arg0.popErrorScope();
            return ret;
        },
        __wbg_prototypesetcall_de8e0d9553586985: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_pushErrorScope_674df658c271daea: function(arg0, arg1) {
            arg0.pushErrorScope(__wbindgen_enum_GpuErrorFilter[arg1]);
        },
        __wbg_push_adb0107829f02d75: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_querySelectorAll_9b6a612499ecb916: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelectorAll(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_queueMicrotask_ac694eae12e92dfb: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_queueMicrotask_be5fe34a8f4cad4d: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queue_3e40156d83b9183e: function(arg0) {
            const ret = arg0.queue;
            return ret;
        },
        __wbg_reason_1460f6c833ca7671: function(arg0, arg1) {
            const ret = arg1.reason;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_reason_a10491741453b0e1: function(arg0) {
            const ret = arg0.reason;
            return (__wbindgen_enum_GpuDeviceLostReason.indexOf(ret) + 1 || 3) - 1;
        },
        __wbg_requestAdapter_245da40985c2fdc5: function(arg0, arg1) {
            const ret = arg0.requestAdapter(arg1);
            return ret;
        },
        __wbg_requestDevice_28434913a23418c4: function(arg0, arg1) {
            const ret = arg0.requestDevice(arg1);
            return ret;
        },
        __wbg_resolveQuerySet_9c9de6862d76dfd5: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.resolveQuerySet(arg1, arg2 >>> 0, arg3 >>> 0, arg4, arg5 >>> 0);
        },
        __wbg_resolve_020f95d838c6ef25: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_send_5f7b516053d59f8d: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.send(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_send_76be66dda8ee844c: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.send(getArrayU8FromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_setBindGroup_14e047cccfca4206: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setBindGroup(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4), arg5, arg6 >>> 0);
        }, arguments); },
        __wbg_setBindGroup_98f0303f15c3cfb4: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setBindGroup(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4), arg5, arg6 >>> 0);
        }, arguments); },
        __wbg_setBindGroup_bc67abae8c962082: function(arg0, arg1, arg2) {
            arg0.setBindGroup(arg1 >>> 0, arg2);
        },
        __wbg_setBindGroup_e7493a4d990b460a: function(arg0, arg1, arg2) {
            arg0.setBindGroup(arg1 >>> 0, arg2);
        },
        __wbg_setInterval_4da81e87aef31371: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.setInterval(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_setPipeline_0c34cc40ab8d6499: function(arg0, arg1) {
            arg0.setPipeline(arg1);
        },
        __wbg_setPipeline_323a115b52180cad: function(arg0, arg1) {
            arg0.setPipeline(arg1);
        },
        __wbg_set_8155bb79a948541b: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_862c439a342a8818: function(arg0, arg1, arg2) {
            arg0.set(arg1, arg2 >>> 0);
        },
        __wbg_set_a_e8353b5faed116b8: function(arg0, arg1) {
            arg0.a = arg1;
        },
        __wbg_set_access_1cc7ab8607a9643c: function(arg0, arg1) {
            arg0.access = __wbindgen_enum_GpuStorageTextureAccess[arg1];
        },
        __wbg_set_alpha_f4d387342be7589e: function(arg0, arg1) {
            arg0.alpha = arg1;
        },
        __wbg_set_alpha_mode_9a10ad8af4fca8f3: function(arg0, arg1) {
            arg0.alphaMode = __wbindgen_enum_GpuCanvasAlphaMode[arg1];
        },
        __wbg_set_alpha_to_coverage_enabled_ae6d838cf7f69c3f: function(arg0, arg1) {
            arg0.alphaToCoverageEnabled = arg1 !== 0;
        },
        __wbg_set_array_layer_count_37c76e4cca82351f: function(arg0, arg1) {
            arg0.arrayLayerCount = arg1 >>> 0;
        },
        __wbg_set_array_stride_be930c5983868c93: function(arg0, arg1) {
            arg0.arrayStride = arg1;
        },
        __wbg_set_aspect_c9292d2a13f954e1: function(arg0, arg1) {
            arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
        },
        __wbg_set_attributes_c0fe49febc11550f: function(arg0, arg1) {
            arg0.attributes = arg1;
        },
        __wbg_set_b_c127e87dfb6c67af: function(arg0, arg1) {
            arg0.b = arg1;
        },
        __wbg_set_base_array_layer_6374493b6bc1a0a9: function(arg0, arg1) {
            arg0.baseArrayLayer = arg1 >>> 0;
        },
        __wbg_set_base_mip_level_5a0524f10a35bff6: function(arg0, arg1) {
            arg0.baseMipLevel = arg1 >>> 0;
        },
        __wbg_set_beginning_of_pass_write_index_aa7255a7590f9493: function(arg0, arg1) {
            arg0.beginningOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_beginning_of_pass_write_index_ac45c363336c24c7: function(arg0, arg1) {
            arg0.beginningOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_binaryType_b701908a03166a9f: function(arg0, arg1) {
            arg0.binaryType = __wbindgen_enum_BinaryType[arg1];
        },
        __wbg_set_bind_group_layouts_b4667372bdcee99f: function(arg0, arg1) {
            arg0.bindGroupLayouts = arg1;
        },
        __wbg_set_binding_0a48264269982c5e: function(arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        },
        __wbg_set_binding_15ab1e2c74990b25: function(arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        },
        __wbg_set_blend_16ab90d22bf8916c: function(arg0, arg1) {
            arg0.blend = arg1;
        },
        __wbg_set_buffer_3b3e4c4a884d1610: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_buffer_ff433f6fc0bcc260: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_buffers_a0aac3bc1d868127: function(arg0, arg1) {
            arg0.buffers = arg1;
        },
        __wbg_set_clear_value_4c76b232d6720cd3: function(arg0, arg1) {
            arg0.clearValue = arg1;
        },
        __wbg_set_code_c616b86ce504e24a: function(arg0, arg1, arg2) {
            arg0.code = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_color_attachments_24458ffe50f4adf3: function(arg0, arg1) {
            arg0.colorAttachments = arg1;
        },
        __wbg_set_color_bfad4ca850b49bef: function(arg0, arg1) {
            arg0.color = arg1;
        },
        __wbg_set_compare_e3e5cfed6b9fb08f: function(arg0, arg1) {
            arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_compute_7c274f1347709d07: function(arg0, arg1) {
            arg0.compute = arg1;
        },
        __wbg_set_count_7e33db45fa773144: function(arg0, arg1) {
            arg0.count = arg1 >>> 0;
        },
        __wbg_set_count_f4ae1546794fb395: function(arg0, arg1) {
            arg0.count = arg1 >>> 0;
        },
        __wbg_set_cull_mode_5a9a78be0b9e959d: function(arg0, arg1) {
            arg0.cullMode = __wbindgen_enum_GpuCullMode[arg1];
        },
        __wbg_set_depth_bias_clamp_6e19879f7d3c4847: function(arg0, arg1) {
            arg0.depthBiasClamp = arg1;
        },
        __wbg_set_depth_bias_d20e6bd7bb8d2943: function(arg0, arg1) {
            arg0.depthBias = arg1;
        },
        __wbg_set_depth_bias_slope_scale_71f49974c86014f3: function(arg0, arg1) {
            arg0.depthBiasSlopeScale = arg1;
        },
        __wbg_set_depth_clear_value_2ca0c53af7b55fd0: function(arg0, arg1) {
            arg0.depthClearValue = arg1;
        },
        __wbg_set_depth_compare_6f8d2861799b9b1b: function(arg0, arg1) {
            arg0.depthCompare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_depth_fail_op_3ffe0a79cc0c6c78: function(arg0, arg1) {
            arg0.depthFailOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_depth_load_op_f93e6e6b73a935f4: function(arg0, arg1) {
            arg0.depthLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_depth_or_array_layers_e21f6b37c67d8790: function(arg0, arg1) {
            arg0.depthOrArrayLayers = arg1 >>> 0;
        },
        __wbg_set_depth_read_only_e7e0fc0fead69d30: function(arg0, arg1) {
            arg0.depthReadOnly = arg1 !== 0;
        },
        __wbg_set_depth_stencil_6aadfcbb91ef3c92: function(arg0, arg1) {
            arg0.depthStencil = arg1;
        },
        __wbg_set_depth_stencil_attachment_f6f91f5bf5d13235: function(arg0, arg1) {
            arg0.depthStencilAttachment = arg1;
        },
        __wbg_set_depth_store_op_9c95e333852d3582: function(arg0, arg1) {
            arg0.depthStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_depth_write_enabled_2321a5fb094805ad: function(arg0, arg1) {
            arg0.depthWriteEnabled = arg1 !== 0;
        },
        __wbg_set_device_997f7098ce1e9a59: function(arg0, arg1) {
            arg0.device = arg1;
        },
        __wbg_set_dimension_117c2064ce996b47: function(arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_dimension_5c6032ac740887c0: function(arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureDimension[arg1];
        },
        __wbg_set_dst_factor_b97f1b6e89186af3: function(arg0, arg1) {
            arg0.dstFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        },
        __wbg_set_end_of_pass_write_index_38e826851cbbb415: function(arg0, arg1) {
            arg0.endOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_end_of_pass_write_index_c60088bc589e6882: function(arg0, arg1) {
            arg0.endOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_entries_bfc700c1f97eec0b: function(arg0, arg1) {
            arg0.entries = arg1;
        },
        __wbg_set_entries_f07df780e3613292: function(arg0, arg1) {
            arg0.entries = arg1;
        },
        __wbg_set_entry_point_24a4e90f52608a39: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_entry_point_77641c5f6ad5355e: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_entry_point_aa503b3bb9fed987: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_fail_op_fa7348fc04abc3f6: function(arg0, arg1) {
            arg0.failOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_format_11c7232d92ed699b: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_3132a1562e48d4f8: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_aa06ccc03e770abb: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_b554909c259d57d4: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuVertexFormat[arg1];
        },
        __wbg_set_format_b8158198b657d617: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_c38221656906581e: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_cb516a1a8d3f7354: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_fragment_5fd7881ebd420d39: function(arg0, arg1) {
            arg0.fragment = arg1;
        },
        __wbg_set_front_face_1adffec645ea35e2: function(arg0, arg1) {
            arg0.frontFace = __wbindgen_enum_GpuFrontFace[arg1];
        },
        __wbg_set_g_130527c5176eefae: function(arg0, arg1) {
            arg0.g = arg1;
        },
        __wbg_set_has_dynamic_offset_4d5601049080763e: function(arg0, arg1) {
            arg0.hasDynamicOffset = arg1 !== 0;
        },
        __wbg_set_height_3ebe4c6ea2510fcc: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_height_ca39bd9597314f83: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_height_d72f2b76484a44de: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_label_0d9081f92dff44a8: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_392dc66ad76d942d: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_3e06143ad04772ae: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_4f44629bc3c49d4b: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_50f397060b5b5610: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_68e2953cfd33a5a5: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_76c4f74a38ff9bcd: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_79484ec4d6d85bbf: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_83c85af4cf247cfd: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_861c8e348e26599d: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_c6d451b2fc960c7d: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_d1b6a326332d0520: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_d687cfb9a30329c8: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_dcf5143835b5d044: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_e345704005fb385b: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_layout_8e94366580ade994: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_layout_b9b36c291ee7f2e1: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_layout_cccbb8f794df887c: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_load_op_4716d76153bebb14: function(arg0, arg1) {
            arg0.loadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_mapped_at_creation_34da9d6bf64b78d6: function(arg0, arg1) {
            arg0.mappedAtCreation = arg1 !== 0;
        },
        __wbg_set_mask_250d8d1991cda6ea: function(arg0, arg1) {
            arg0.mask = arg1 >>> 0;
        },
        __wbg_set_min_binding_size_9389ad67218af140: function(arg0, arg1) {
            arg0.minBindingSize = arg1;
        },
        __wbg_set_mip_level_count_ce77bbcd6aa77dfb: function(arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        },
        __wbg_set_mip_level_count_faa8a47d0fd87c1e: function(arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        },
        __wbg_set_module_51b2f2e08accb016: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_module_5f33a55198ad797f: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_module_92f53f6a4172b60c: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_multisample_8fea65aa177ce42b: function(arg0, arg1) {
            arg0.multisample = arg1;
        },
        __wbg_set_multisampled_b526741755338725: function(arg0, arg1) {
            arg0.multisampled = arg1 !== 0;
        },
        __wbg_set_offset_1a0f95ffb7dd6f40: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_offset_7742a652907a0dcc: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_onclose_cb71fea4ad9056fc: function(arg0, arg1) {
            arg0.onclose = arg1;
        },
        __wbg_set_onerror_f1491b13f0fea022: function(arg0, arg1) {
            arg0.onerror = arg1;
        },
        __wbg_set_onmessage_065a797dafa9c437: function(arg0, arg1) {
            arg0.onmessage = arg1;
        },
        __wbg_set_onopen_b28699f3431204cf: function(arg0, arg1) {
            arg0.onopen = arg1;
        },
        __wbg_set_operation_3f77d077d89e8104: function(arg0, arg1) {
            arg0.operation = __wbindgen_enum_GpuBlendOperation[arg1];
        },
        __wbg_set_pass_op_f289598b9bf2b26f: function(arg0, arg1) {
            arg0.passOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_power_preference_915480f4b9565dc2: function(arg0, arg1) {
            arg0.powerPreference = __wbindgen_enum_GpuPowerPreference[arg1];
        },
        __wbg_set_primitive_65f0197724aa1998: function(arg0, arg1) {
            arg0.primitive = arg1;
        },
        __wbg_set_query_set_0a78c3dcb3650b2b: function(arg0, arg1) {
            arg0.querySet = arg1;
        },
        __wbg_set_query_set_3088af736d5ed6bb: function(arg0, arg1) {
            arg0.querySet = arg1;
        },
        __wbg_set_r_1f5bdc587af1ad2e: function(arg0, arg1) {
            arg0.r = arg1;
        },
        __wbg_set_required_features_42347bf311233eb6: function(arg0, arg1) {
            arg0.requiredFeatures = arg1;
        },
        __wbg_set_resolve_target_d9752c5e8b620b01: function(arg0, arg1) {
            arg0.resolveTarget = arg1;
        },
        __wbg_set_resource_f2d72f59cc9308fc: function(arg0, arg1) {
            arg0.resource = arg1;
        },
        __wbg_set_sample_count_47378e3363905cfe: function(arg0, arg1) {
            arg0.sampleCount = arg1 >>> 0;
        },
        __wbg_set_sample_type_6d1e240a417bdf44: function(arg0, arg1) {
            arg0.sampleType = __wbindgen_enum_GpuTextureSampleType[arg1];
        },
        __wbg_set_sampler_f864a162bad4f66f: function(arg0, arg1) {
            arg0.sampler = arg1;
        },
        __wbg_set_shader_location_512e18558f4b5044: function(arg0, arg1) {
            arg0.shaderLocation = arg1 >>> 0;
        },
        __wbg_set_size_657d97f8d513b5e9: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_size_6b2fc4a0e39e4d07: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_size_c78ae8d2e2181815: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_src_factor_f6d030d42c740795: function(arg0, arg1) {
            arg0.srcFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        },
        __wbg_set_stencil_back_445dca081a7a482f: function(arg0, arg1) {
            arg0.stencilBack = arg1;
        },
        __wbg_set_stencil_clear_value_1ff6e0286bac04f0: function(arg0, arg1) {
            arg0.stencilClearValue = arg1 >>> 0;
        },
        __wbg_set_stencil_front_c2e7583fc42d1289: function(arg0, arg1) {
            arg0.stencilFront = arg1;
        },
        __wbg_set_stencil_load_op_aa4f07acdc265e20: function(arg0, arg1) {
            arg0.stencilLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_stencil_read_mask_fa2e080bf4e50296: function(arg0, arg1) {
            arg0.stencilReadMask = arg1 >>> 0;
        },
        __wbg_set_stencil_read_only_c668e2f4792200dc: function(arg0, arg1) {
            arg0.stencilReadOnly = arg1 !== 0;
        },
        __wbg_set_stencil_store_op_817f5569dcbe011c: function(arg0, arg1) {
            arg0.stencilStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_stencil_write_mask_cd78f765a9d1922b: function(arg0, arg1) {
            arg0.stencilWriteMask = arg1 >>> 0;
        },
        __wbg_set_step_mode_957a94d543f8cb9c: function(arg0, arg1) {
            arg0.stepMode = __wbindgen_enum_GpuVertexStepMode[arg1];
        },
        __wbg_set_storage_texture_c3919f22b211c542: function(arg0, arg1) {
            arg0.storageTexture = arg1;
        },
        __wbg_set_store_op_cbb1498982c43f58: function(arg0, arg1) {
            arg0.storeOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_strip_index_format_cf0e1ae7ebc6e801: function(arg0, arg1) {
            arg0.stripIndexFormat = __wbindgen_enum_GpuIndexFormat[arg1];
        },
        __wbg_set_targets_ed82ec017a08e9be: function(arg0, arg1) {
            arg0.targets = arg1;
        },
        __wbg_set_texture_bf820de044f0d291: function(arg0, arg1) {
            arg0.texture = arg1;
        },
        __wbg_set_timestamp_writes_2c9801ffc2c74de7: function(arg0, arg1) {
            arg0.timestampWrites = arg1;
        },
        __wbg_set_timestamp_writes_b9e1d87e2f057bd1: function(arg0, arg1) {
            arg0.timestampWrites = arg1;
        },
        __wbg_set_topology_e97a4999800a37a9: function(arg0, arg1) {
            arg0.topology = __wbindgen_enum_GpuPrimitiveTopology[arg1];
        },
        __wbg_set_type_40f4ae4fa32946cd: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuBufferBindingType[arg1];
        },
        __wbg_set_type_4f1cd48d79f4d6dc: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuSamplerBindingType[arg1];
        },
        __wbg_set_type_90c8feb4413481b1: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuQueryType[arg1];
        },
        __wbg_set_usage_4810d9c6b8041aa6: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_794d488202743c10: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_9aa23fa1e13799a8: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_ba31cd3d9ce977fe: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_vertex_90d5407453f59d4a: function(arg0, arg1) {
            arg0.vertex = arg1;
        },
        __wbg_set_view_36f140b43e2eca60: function(arg0, arg1) {
            arg0.view = arg1;
        },
        __wbg_set_view_51a85be8f2338ed2: function(arg0, arg1) {
            arg0.view = arg1;
        },
        __wbg_set_view_dimension_36c0bf530395d014: function(arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_view_dimension_553cd9fa176d06ca: function(arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_view_formats_0a8a8e11cfa73759: function(arg0, arg1) {
            arg0.viewFormats = arg1;
        },
        __wbg_set_view_formats_79f521105f1c4c8c: function(arg0, arg1) {
            arg0.viewFormats = arg1;
        },
        __wbg_set_visibility_eef2d8e9608a8981: function(arg0, arg1) {
            arg0.visibility = arg1 >>> 0;
        },
        __wbg_set_width_36ef6630b22fc519: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_width_60b542bb7870a825: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_width_661c95ea46b71eba: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_write_mask_6d312328ac2d4d97: function(arg0, arg1) {
            arg0.writeMask = arg1 >>> 0;
        },
        __wbg_static_accessor_GLOBAL_THIS_466428f93b4eaa76: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_c7aea38d4de089bc: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_42d4fae05e59267a: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_e0db14a0eba6a812: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_submit_2521bdd9a232bca7: function(arg0, arg1) {
            arg0.submit(arg1);
        },
        __wbg_then_03887f33d8da4f96: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_then_7026b513a94278a8: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_then_72819b8d4e081fb5: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_f730e17d115c60b2: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_unmap_815a075fd850cb73: function(arg0) {
            arg0.unmap();
        },
        __wbg_width_1952934caca67137: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_writeBuffer_e8b792fb0962f30d: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.writeBuffer(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 236, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___wasm_bindgen_622e9445b6bb02a6___JsValue______true_);
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 273, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___wasm_bindgen_622e9445b6bb02a6___JsValue__core_2f68d491536bd501___result__Result_____wasm_bindgen_622e9445b6bb02a6___JsError___true_);
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("CloseEvent")], shim_idx: 205, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true_);
            return ret;
        },
        __wbindgen_cast_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("Event")], shim_idx: 205, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true__3);
            return ret;
        },
        __wbindgen_cast_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 205, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true__4);
            return ret;
        },
        __wbindgen_cast_0000000000000006: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 209, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke_______true_);
            return ret;
        },
        __wbindgen_cast_0000000000000007: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000008: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_cast_0000000000000009: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./what_is_this_bg.js": import0,
    };
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke_______true_(arg0, arg1) {
    wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke_______true_(arg0, arg1);
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___wasm_bindgen_622e9445b6bb02a6___JsValue______true_(arg0, arg1, arg2) {
    wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___wasm_bindgen_622e9445b6bb02a6___JsValue______true_(arg0, arg1, arg2);
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true_(arg0, arg1, arg2) {
    wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true_(arg0, arg1, arg2);
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true__3(arg0, arg1, arg2) {
    wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true__3(arg0, arg1, arg2);
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true__4(arg0, arg1, arg2) {
    wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___web_sys_c63c573408ed35de___features__gen_CloseEvent__CloseEvent______true__4(arg0, arg1, arg2);
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___wasm_bindgen_622e9445b6bb02a6___JsValue__core_2f68d491536bd501___result__Result_____wasm_bindgen_622e9445b6bb02a6___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___wasm_bindgen_622e9445b6bb02a6___JsValue__core_2f68d491536bd501___result__Result_____wasm_bindgen_622e9445b6bb02a6___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___js_sys_bc21f7f944e8537f___Function_fn_wasm_bindgen_622e9445b6bb02a6___JsValue_____wasm_bindgen_622e9445b6bb02a6___sys__Undefined___js_sys_bc21f7f944e8537f___Function_fn_wasm_bindgen_622e9445b6bb02a6___JsValue_____wasm_bindgen_622e9445b6bb02a6___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen_622e9445b6bb02a6___convert__closures_____invoke___js_sys_bc21f7f944e8537f___Function_fn_wasm_bindgen_622e9445b6bb02a6___JsValue_____wasm_bindgen_622e9445b6bb02a6___sys__Undefined___js_sys_bc21f7f944e8537f___Function_fn_wasm_bindgen_622e9445b6bb02a6___JsValue_____wasm_bindgen_622e9445b6bb02a6___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_BinaryType = ["blob", "arraybuffer"];


const __wbindgen_enum_GpuBlendFactor = ["zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"];


const __wbindgen_enum_GpuBlendOperation = ["add", "subtract", "reverse-subtract", "min", "max"];


const __wbindgen_enum_GpuBufferBindingType = ["uniform", "storage", "read-only-storage"];


const __wbindgen_enum_GpuCanvasAlphaMode = ["opaque", "premultiplied"];


const __wbindgen_enum_GpuCompareFunction = ["never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"];


const __wbindgen_enum_GpuCullMode = ["none", "front", "back"];


const __wbindgen_enum_GpuDeviceLostReason = ["unknown", "destroyed"];


const __wbindgen_enum_GpuErrorFilter = ["validation", "out-of-memory", "internal"];


const __wbindgen_enum_GpuFrontFace = ["ccw", "cw"];


const __wbindgen_enum_GpuIndexFormat = ["uint16", "uint32"];


const __wbindgen_enum_GpuLoadOp = ["load", "clear"];


const __wbindgen_enum_GpuPowerPreference = ["low-power", "high-performance"];


const __wbindgen_enum_GpuPrimitiveTopology = ["point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"];


const __wbindgen_enum_GpuQueryType = ["occlusion", "timestamp"];


const __wbindgen_enum_GpuSamplerBindingType = ["filtering", "non-filtering", "comparison"];


const __wbindgen_enum_GpuStencilOperation = ["keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"];


const __wbindgen_enum_GpuStorageTextureAccess = ["write-only", "read-only", "read-write"];


const __wbindgen_enum_GpuStoreOp = ["store", "discard"];


const __wbindgen_enum_GpuTextureAspect = ["all", "stencil-only", "depth-only"];


const __wbindgen_enum_GpuTextureDimension = ["1d", "2d", "3d"];


const __wbindgen_enum_GpuTextureFormat = ["r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"];


const __wbindgen_enum_GpuTextureSampleType = ["float", "unfilterable-float", "depth", "sint", "uint"];


const __wbindgen_enum_GpuTextureViewDimension = ["1d", "2d", "2d-array", "cube", "cube-array", "3d"];


const __wbindgen_enum_GpuVertexFormat = ["uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"];


const __wbindgen_enum_GpuVertexStepMode = ["vertex", "instance"];

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('what_is_this_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
