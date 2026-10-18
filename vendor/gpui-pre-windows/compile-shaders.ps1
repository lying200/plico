# WSL Release shader build, using the same Direct3D compiler and shader targets as upstream.
param([string]$SourceDirectory, [string]$OutputFile)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ShaderCompiler {
    [ComImport, Guid("8BA5FB08-5195-40e2-AC58-0D989C3A0102"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface ID3DBlob {
        [PreserveSig] IntPtr GetBufferPointer();
        [PreserveSig] UIntPtr GetBufferSize();
    }
    [DllImport("d3dcompiler_47.dll", CharSet = CharSet.Ansi)]
    static extern int D3DCompile(byte[] data, UIntPtr size, string name, IntPtr defines,
        IntPtr include, string entry, string target, uint flags1, uint flags2,
        out ID3DBlob code, out ID3DBlob errors);
    public static byte[] Compile(string source, string entry, string target) {
        byte[] data = System.Text.Encoding.UTF8.GetBytes(source);
        ID3DBlob code = null, errors = null;
        try {
            int result = D3DCompile(data, (UIntPtr)data.Length, "gpui.hlsl", IntPtr.Zero,
                IntPtr.Zero, entry, target, 32768, 0, out code, out errors);
            if (result < 0) throw new Exception(errors == null ? "D3DCompile failed" : Marshal.PtrToStringAnsi(errors.GetBufferPointer()));
            byte[] bytes = new byte[(int)code.GetBufferSize().ToUInt64()];
            Marshal.Copy(code.GetBufferPointer(), bytes, 0, bytes.Length);
            return bytes;
        } finally {
            if (code != null) Marshal.ReleaseComObject(code);
            if (errors != null) Marshal.ReleaseComObject(errors);
        }
    }
}
'@
$include = [IO.File]::ReadAllText((Join-Path $SourceDirectory 'alpha_correction.hlsl'))
$modules = 'quad','shadow','path_rasterization','path_sprite','underline','monochrome_sprite','subpixel_sprite','polychrome_sprite','emoji_rasterization'
$builder = New-Object System.Text.StringBuilder
foreach ($module in $modules) {
    $file = if ($module -eq 'emoji_rasterization') {'color_text_raster.hlsl'} else {'shaders.hlsl'}
    $source = [IO.File]::ReadAllText((Join-Path $SourceDirectory $file)).Replace('#include "alpha_correction.hlsl"', $include)
    foreach ($stage in @(@('vertex','vs_4_1'), @('fragment','ps_4_1'))) {
        $bytes = [ShaderCompiler]::Compile($source, ($module + '_' + $stage[0]), $stage[1])
        $name = ($module + '_' + $stage[0] + '_BYTES').ToUpperInvariant()
        [void]$builder.AppendLine(('const ' + $name + ': &[u8] = &[' + ($bytes -join ',') + '];'))
    }
}
[IO.File]::WriteAllText($OutputFile, $builder.ToString(), (New-Object System.Text.UTF8Encoding $false))
