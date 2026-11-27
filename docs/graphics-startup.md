# Graphics startup — ACID RECALL 1.1

[Official website](https://acidrecall.com)

On Windows, version 1.1 uses **DirectX 12 (DX12)** by default. Extract the ZIP and double-click
**ACID-RECALL.exe**. No PowerShell command or graphics preference is needed for normal startup.

This avoids initializing Vulkan on Intel drivers that can crash before the first window appears.
The reported `igvk64.dll` crash was bypassed by the user's `WGPU_BACKEND=dx12` workaround.
The default is now configured directly on the graphics instance, before adapter enumeration;
the application does not change system environment variables or install drivers.

## Manual backend selection

From PowerShell in the extracted application folder:

```powershell
& '.\ACID-RECALL.exe' --graphics dx12
```

`--graphics vulkan` selects Vulkan explicitly. `--graphics auto` restores the graphics library's
automatic API selection and can initialize Vulkan again; avoid both on the affected Intel laptop.
These are launch options, so they can also be appended to a Windows shortcut's Target field.

Priority: **--graphics > WGPU_BACKEND > Windows DX12 default**. Existing advanced setups such as
`$env:WGPU_BACKEND = 'dx12'` continue to work. A valid inherited `WGPU_BACKEND=vulkan` still requests
Vulkan; use `--graphics dx12` to override it. `--help` lists the public graphics option.

macOS and Linux retain their original graphics defaults unless explicitly overridden.
Graphics selection does not change ASIO/WASAPI, audio buffers, MIDI sync or saved projects.

## Useful diagnostics

```powershell
$env:ORB_GPU_INFO = '1'
& '.\ACID-RECALL.exe' --graphics dx12
```

The console reports the selected graphics API and adapter. A default Windows launch should
report `gpu: Dx12`. If startup still fails, report the Windows version, graphics adapter and
driver version, launch options, and the Windows Reliability Monitor faulting module.

A native graphics-driver access violation can terminate the process before ordinary error
handling or an in-process fallback is possible. Version 1.1 avoids Vulkan by default on Windows;
it does not claim to repair the driver or guarantee compatibility with every graphics device.
