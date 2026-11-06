#Requires -RunAsAdministrator
<#
.SYNOPSIS
  Creates the isolated Hyper-V test VM for the Phase 5 RDP input feasibility study (docs/PHASE5_RDP_INPUT_FEASIBILITY.md 3.2).

.DESCRIPTION
  Owner requirements (2026-11-06): 4 GB RAM, 4 vCPU capped at 80%, dynamic VHDX max 20 GB, and the VM may talk to the host ONLY.
  No external network access. The VM is NOT started by this script.

  Isolation design:
    * a dedicated Hyper-V Internal switch (host <-> VM only), no external adapter, no NAT, no Internet Connection Sharing, no bridge;
    * the host side of the switch gets a fixed address with NO default gateway and IP forwarding disabled;
    * the VM gets a fixed address by hand inside the guest, with NO gateway and NO DNS;
    * the script refuses to continue if a NetNat object exists for the test subnet.
  This is a test VM with a synthetic, throwaway account only. Delete it with -Remove when finished. It is not a product component.
  Elevated PowerShell is required (Hyper-V cmdlets); nothing here bypasses any security feature.

.PARAMETER IsoPath
  Full path of the Windows Server 2022 evaluation ISO (required; no default so no user name is stored in the repository).

.PARAMETER Root
  Folder for the VM files. A new sub-folder is created; existing unrelated folders are left alone.
#>
[CmdletBinding()]
param(
    [string]$IsoPath,
    [string]$Root = 'S:\temp\kagilink-rdp-test',
    [string]$VmName = 'KL-RdpTest',
    [string]$SwitchName = 'KL-RdpTest-HostOnly',
    [string]$HostIp = '192.168.77.1',
    [int]$PrefixLength = 24,
    [switch]$Remove
)

$ErrorActionPreference = 'Stop'

function Get-VmIfExists([string]$name) { Get-VM -Name $name -ErrorAction SilentlyContinue }

if ($Remove) {
    $vm = Get-VmIfExists $VmName
    if ($vm) {
        if ($vm.State -ne 'Off') { Stop-VM -Name $VmName -TurnOff -Force }
        Remove-VM -Name $VmName -Force
    }
    if (Test-Path $Root) {
        # Only the folder this script created, and only when it is the expected leaf.
        if ((Split-Path $Root -Leaf) -eq 'kagilink-rdp-test') { Remove-Item -LiteralPath $Root -Recurse -Force }
    }
    if (Get-VMSwitch -Name $SwitchName -ErrorAction SilentlyContinue) { Remove-VMSwitch -Name $SwitchName -Force }
    Write-Host 'Removed VM, files and switch.'
    return
}

if (-not $IsoPath -or -not (Test-Path -LiteralPath $IsoPath -PathType Leaf)) { throw 'Pass -IsoPath with the full path of the evaluation ISO.' }
if (Get-VmIfExists $VmName) { throw "A VM named $VmName already exists." }

$subnetPrefix = ($HostIp -replace '\.\d+$', '.0')
if (Get-NetNat -ErrorAction SilentlyContinue | Where-Object { $_.InternalIPInterfaceAddressPrefix -like "$subnetPrefix/*" }) {
    throw "A NAT exists for $subnetPrefix. Refusing: the test network must have no NAT."
}

if (-not (Test-Path $Root)) { New-Item -ItemType Directory -Path $Root | Out-Null }

# 1. Host-only switch. Internal = host <-> VM, never an external adapter.
if (-not (Get-VMSwitch -Name $SwitchName -ErrorAction SilentlyContinue)) {
    New-VMSwitch -Name $SwitchName -SwitchType Internal | Out-Null
}
$sw = Get-VMSwitch -Name $SwitchName
if ($sw.SwitchType -ne 'Internal') { throw "Switch $SwitchName is not Internal. Refusing." }

# 2. Host side address: no gateway, no forwarding, no DNS.
$alias = "vEthernet ($SwitchName)"
$ifc = Get-NetIPInterface -InterfaceAlias $alias -AddressFamily IPv4
Set-NetIPInterface -InterfaceAlias $alias -AddressFamily IPv4 -Forwarding Disabled
Get-NetIPAddress -InterfaceAlias $alias -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -ne $HostIp } | Remove-NetIPAddress -Confirm:$false
if (-not (Get-NetIPAddress -InterfaceAlias $alias -IPAddress $HostIp -ErrorAction SilentlyContinue)) {
    New-NetIPAddress -InterfaceAlias $alias -IPAddress $HostIp -PrefixLength $PrefixLength | Out-Null   # no -DefaultGateway on purpose
}
Set-DnsClientServerAddress -InterfaceAlias $alias -ResetServerAddresses

# 3. The VM: generation 2, 4 GB static RAM, 4 vCPU capped at 80%, dynamic 20 GB disk, no checkpoints.
$vhd = Join-Path $Root "$VmName.vhdx"
New-VM -Name $VmName -Generation 2 -MemoryStartupBytes 4GB -NewVHDPath $vhd -NewVHDSizeBytes 20GB -SwitchName $SwitchName -Path $Root | Out-Null
Set-VMMemory -VMName $VmName -DynamicMemoryEnabled $false
Set-VMProcessor -VMName $VmName -Count 4 -Maximum 80
Set-VM -Name $VmName -AutomaticCheckpointsEnabled $false -CheckpointType Disabled
$dvd = Add-VMDvdDrive -VMName $VmName -Path $IsoPath -Passthru
Set-VMFirmware -VMName $VmName -FirstBootDevice $dvd

# 4. Final isolation check: exactly one adapter, on the Internal switch.
$nics = Get-VMNetworkAdapter -VMName $VmName
if ($nics.Count -ne 1 -or $nics[0].SwitchName -ne $SwitchName) { throw 'Unexpected network adapter layout. Remove the VM and investigate.' }

Write-Host "Created $VmName (not started). Disk: $vhd (dynamic, max 20 GB). Switch: $SwitchName (Internal, host $HostIp, no gateway)."
Write-Host ''
Write-Host 'Next steps (by hand, then record results in the private notes):'
Write-Host '  1. Start-VM, install Windows Server 2022 Standard Evaluation (Server Core), set a throwaway synthetic Administrator password.'
Write-Host '  2. In the guest set a fixed address 192.168.77.2/24 with NO gateway and NO DNS.'
Write-Host '  3. Isolation proof inside the guest: ping 192.168.77.1 must work; ping 1.1.1.1, ping 8.8.8.8 and a DNS lookup must FAIL.'
Write-Host '     On the host: no route from the vEthernet adapter to the Internet, Get-NetNat returns nothing for 192.168.77.0.'
Write-Host '  4. Only after the proof, continue with docs/PHASE5_RDP_INPUT_FEASIBILITY.md 3.2. Remove everything with -Remove.'
