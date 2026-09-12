use super::*;

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        let mut device = ptr::null_mut();
        let mut context = ptr::null_mut();
        let mut duplication = ptr::null_mut();
        #[allow(invalid_value)]
        let mut desc = unsafe { mem::MaybeUninit::uninit().assume_init() };
        #[allow(invalid_value)]
        let mut adapter_desc1 = unsafe { mem::MaybeUninit::uninit().assume_init() };
        let mut gdi_capturer = None;

        let mut res = if display.gdi {
            wrap_hresult(1)
        } else {
            let res = wrap_hresult(unsafe {
                D3D11CreateDevice(
                    display.adapter.0 as *mut _,
                    D3D_DRIVER_TYPE_UNKNOWN,
                    ptr::null_mut(), // No software rasterizer.
                    0,               // No device flags.
                    ptr::null_mut(), // Feature levels.
                    0,               // Feature levels' length.
                    D3D11_SDK_VERSION,
                    &mut device,
                    ptr::null_mut(),
                    &mut context,
                )
            });
            if res.is_ok() {
                wrap_hresult(unsafe { (*display.adapter.0).GetDesc1(&mut adapter_desc1) })
            } else {
                res
            }
        };
        let device = ComPtr(device);
        let context = ComPtr(context);

        if res.is_err() {
            gdi_capturer = display.create_gdi();
            println!("Fallback to GDI");
            if gdi_capturer.is_some() {
                res = Ok(());
            }
        } else {
            res = wrap_hresult(unsafe {
                let hres = (*display.inner.0).DuplicateOutput(device.0 as *mut _, &mut duplication);
                if hres != S_OK {
                    gdi_capturer = display.create_gdi();
                    println!("Fallback to GDI");
                    if gdi_capturer.is_some() {
                        S_OK
                    } else {
                        hres
                    }
                } else {
                    hres
                }

                // NVFBC(NVIDIA Capture SDK) which xpra used already deprecated, https://developer.nvidia.com/capture-sdk

                // also try high version DXGI for better performance, e.g.
                // https://docs.microsoft.com/zh-cn/windows/win32/direct3ddxgi/dxgi-1-2-improvements
                // dxgi-1-6 may too high, only support win10 (2018)
                // https://docs.microsoft.com/zh-cn/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format
                // DXGI_FORMAT_420_OPAQUE
                // IDXGIOutputDuplication::GetFrameDirtyRects and IDXGIOutputDuplication::GetFrameMoveRects
                // can help us update screen incrementally

                /* // not supported on my PC, try in the future
                use winapi::shared::dxgiformat::DXGI_FORMAT_B8G8R8A8_UNORM;

                let format : Vec<DXGI_FORMAT> = vec![DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_420_OPAQUE];
                (*display.inner).DuplicateOutput1(
                    device as *mut _,
                    0 as UINT,
                    2 as UINT,
                    format.as_ptr(),
                    &mut duplication
                )
                */

                // if above not work, I think below should not work either, try later
                // https://developer.nvidia.com/capture-sdk deprecated
                // examples using directx + nvideo sdk for GPU-accelerated video encoding/decoding
                // https://github.com/NVIDIA/video-sdk-samples
            });
        }

        res?;

        if !duplication.is_null() {
            unsafe {
                (*duplication).GetDesc(&mut desc);
            }
        }
        let rotate = Self::create_rotations(device.0, context.0, &display);

        Ok(Capturer {
            device,
            context,
            duplication: ComPtr(duplication),
            fastlane: desc.DesktopImageInSystemMemory == TRUE,
            surface: ComPtr(ptr::null_mut()),
            texture: ComPtr(ptr::null_mut()),
            width: display.width() as usize,
            height: display.height() as usize,
            display,
            rotated: Vec::new(),
            gdi_capturer,
            gdi_buffer: Vec::new(),
            saved_raw_data: Vec::new(),
            output_texture: false,
            adapter_desc1,
            rotate,
        })
    }

    pub(super) fn create_rotations(
        device: *mut ID3D11Device,
        context: *mut ID3D11DeviceContext,
        display: &Display,
    ) -> Rotate {
        let mut video_context: *mut ID3D11VideoContext = ptr::null_mut();
        let mut video_device: *mut ID3D11VideoDevice = ptr::null_mut();
        let mut video_processor_enum: *mut ID3D11VideoProcessorEnumerator = ptr::null_mut();
        let mut video_processor: *mut ID3D11VideoProcessor = ptr::null_mut();
        let processor_rotation = match display.rotation() {
            DXGI_MODE_ROTATION_ROTATE90 => Some(D3D11_VIDEO_PROCESSOR_ROTATION_90),
            DXGI_MODE_ROTATION_ROTATE180 => Some(D3D11_VIDEO_PROCESSOR_ROTATION_180),
            DXGI_MODE_ROTATION_ROTATE270 => Some(D3D11_VIDEO_PROCESSOR_ROTATION_270),
            _ => None,
        };
        if let Some(processor_rotation) = processor_rotation {
            println!("create rotations");
            if !device.is_null() && !context.is_null() {
                unsafe {
                    (*context).QueryInterface(
                        &IID_ID3D11VideoContext,
                        &mut video_context as *mut *mut _ as *mut *mut _,
                    );
                    if !video_context.is_null() {
                        (*device).QueryInterface(
                            &IID_ID3D11VideoDevice,
                            &mut video_device as *mut *mut _ as *mut *mut _,
                        );
                        if !video_device.is_null() {
                            let (input_width, input_height) = match display.rotation() {
                                DXGI_MODE_ROTATION_ROTATE90 | DXGI_MODE_ROTATION_ROTATE270 => {
                                    (display.height(), display.width())
                                }
                                _ => (display.width(), display.height()),
                            };
                            let (output_width, output_height) = (display.width(), display.height());
                            let content_desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                                InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                                InputFrameRate: DXGI_RATIONAL {
                                    Numerator: 30,
                                    Denominator: 1,
                                },
                                InputWidth: input_width as _,
                                InputHeight: input_height as _,
                                OutputFrameRate: DXGI_RATIONAL {
                                    Numerator: 30,
                                    Denominator: 1,
                                },
                                OutputWidth: output_width as _,
                                OutputHeight: output_height as _,
                                Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
                            };
                            (*video_device).CreateVideoProcessorEnumerator(
                                &content_desc,
                                &mut video_processor_enum,
                            );
                            if !video_processor_enum.is_null() {
                                let mut caps: D3D11_VIDEO_PROCESSOR_CAPS = mem::zeroed();
                                if S_OK == (*video_processor_enum).GetVideoProcessorCaps(&mut caps)
                                {
                                    if caps.FeatureCaps
                                        & D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ROTATION
                                        != 0
                                    {
                                        (*video_device).CreateVideoProcessor(
                                            video_processor_enum,
                                            0,
                                            &mut video_processor,
                                        );
                                        if !video_processor.is_null() {
                                            (*video_context).VideoProcessorSetStreamRotation(
                                                video_processor,
                                                0,
                                                TRUE,
                                                processor_rotation,
                                            );
                                            (*video_context)
                                                .VideoProcessorSetStreamAutoProcessingMode(
                                                    video_processor,
                                                    0,
                                                    FALSE,
                                                );
                                            (*video_context).VideoProcessorSetStreamFrameFormat(
                                                video_processor,
                                                0,
                                                D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                                            );
                                            (*video_context).VideoProcessorSetStreamSourceRect(
                                                video_processor,
                                                0,
                                                TRUE,
                                                &RECT {
                                                    left: 0,
                                                    top: 0,
                                                    right: input_width as _,
                                                    bottom: input_height as _,
                                                },
                                            );
                                            (*video_context).VideoProcessorSetStreamDestRect(
                                                video_processor,
                                                0,
                                                TRUE,
                                                &RECT {
                                                    left: 0,
                                                    top: 0,
                                                    right: output_width as _,
                                                    bottom: output_height as _,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let video_context = ComPtr(video_context);
        let video_device = ComPtr(video_device);
        let video_processor_enum = ComPtr(video_processor_enum);
        let video_processor = ComPtr(video_processor);
        let rotated_texture = ComPtr(ptr::null_mut());
        Rotate {
            video_context,
            video_device,
            video_processor_enum,
            video_processor,
            texture: (rotated_texture, false),
        }
    }
}
