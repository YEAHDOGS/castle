<#
.SYNOPSIS
    Castle VM -- Native PowerShell WPF Graphical User Interface.
.DESCRIPTION
    Provides a lightweight, zero-dependency desktop UI for the Castle VM pipeline.
#>

Add-Type -AssemblyName PresentationFramework

$PowershellDir = (Get-Command pwsh).Source
if (-not $PowershellDir) {
    $PowershellDir = "powershell.exe"
}

$Xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="Castle VM Manager" Height="600" Width="900"
        Background="#1E1E1E" Foreground="#CCCCCC" WindowStartupLocation="CenterScreen">
    <Window.Resources>
        <!-- Button Style -->
        <Style TargetType="Button">
            <Setter Property="Background" Value="#333333"/>
            <Setter Property="Foreground" Value="White"/>
            <Setter Property="Padding" Value="10,8"/>
            <Setter Property="Margin" Value="0,5"/>
            <Setter Property="BorderThickness" Value="0"/>
            <Setter Property="FontFamily" Value="Consolas, Segoe UI"/>
            <Setter Property="Template">
                <Setter.Value>
                    <ControlTemplate TargetType="Button">
                        <Border Background="{TemplateBinding Background}" CornerRadius="4" Padding="{TemplateBinding Padding}">
                            <ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center"/>
                        </Border>
                    </ControlTemplate>
                </Setter.Value>
            </Setter>
            <Style.Triggers>
                <Trigger Property="IsMouseOver" Value="True">
                    <Setter Property="Background" Value="#555555"/>
                </Trigger>
                <Trigger Property="IsPressed" Value="True">
                    <Setter Property="Background" Value="#007ACC"/>
                </Trigger>
            </Style.Triggers>
        </Style>
    </Window.Resources>
    
    <Grid Margin="15">
        <Grid.ColumnDefinitions>
            <ColumnDefinition Width="*"/>
            <ColumnDefinition Width="250"/>
        </Grid.ColumnDefinitions>
        
        <GroupBox Header="VM Targets Registry" Foreground="#00E5FF" BorderBrush="#444444" Grid.Column="0" Margin="0,0,15,0" Padding="5" FontSize="14" FontFamily="Segoe UI Semibold">
            <ListView x:Name="TargetListView" Background="#252526" Foreground="White" BorderThickness="0" Margin="5" FontFamily="Consolas">
                <ListView.ItemContainerStyle>
                    <Style TargetType="ListViewItem">
                        <Setter Property="HorizontalContentAlignment" Value="Stretch"/>
                        <Setter Property="Padding" Value="5,8"/>
                        <Style.Triggers>
                            <Trigger Property="IsSelected" Value="True">
                                <Setter Property="Background" Value="#007ACC"/>
                            </Trigger>
                        </Style.Triggers>
                    </Style>
                </ListView.ItemContainerStyle>
                <ListView.View>
                    <GridView>
                        <GridViewColumn Header="ID" DisplayMemberBinding="{Binding Id}" Width="150"/>
                        <GridViewColumn Header="Target Name" DisplayMemberBinding="{Binding Name}" Width="280"/>
                        <GridViewColumn Header="OS" DisplayMemberBinding="{Binding OsFamily}" Width="80"/>
                        <GridViewColumn Header="Disk" DisplayMemberBinding="{Binding DiskSize}" Width="60"/>
                    </GridView>
                </ListView.View>
            </ListView>
        </GroupBox>

        <StackPanel Grid.Column="1" Orientation="Vertical">
            <GroupBox Header="Boot Actions" Foreground="#00E5FF" BorderBrush="#444444" Margin="0,0,0,15" Padding="10" FontSize="14" FontFamily="Segoe UI Semibold">
                <StackPanel>
                    <Button x:Name="BtnLaunch" Content="▶ Launch Interactive" Background="#007ACC" FontWeight="Bold"/>
                    <Button x:Name="BtnLaunchBg" Content="🚀 Launch Background" Background="#2E8B57" FontWeight="Bold"/>
                    <Button x:Name="BtnLaunchVnc" Content="🖥 Launch with VNC" Background="#D2691E"/>
                </StackPanel>
            </GroupBox>

            <GroupBox Header="Cleanup &amp; Maintenance" Foreground="#00E5FF" BorderBrush="#444444" Padding="10" FontSize="14" FontFamily="Segoe UI Semibold">
                <StackPanel>
                    <Button x:Name="BtnDeleteDisk" Content="🗑 Delete Virtual Disk"/>
                    <Button x:Name="BtnDeleteIso" Content="🗑 Delete Cached ISO"/>
                    <Button x:Name="BtnPurge" Content="⚠ Full Purge (Reset)" Background="#8B0000" FontWeight="Bold"/>
                </StackPanel>
            </GroupBox>

            <TextBlock x:Name="TxtStatus" Text="Ready. Select a VM target and an action." Foreground="#AAAAAA" TextWrapping="Wrap" Margin="5,20,5,5" FontSize="12" FontFamily="Consolas"/>
        </StackPanel>
    </Grid>
</Window>
"@

$Reader = (New-Object System.Xml.XmlNodeReader ([xml]$Xaml))
$Window = [Windows.Markup.XamlReader]::Load($Reader)

# UI Elements Mapping
$TargetListView = $Window.FindName("TargetListView")
$BtnLaunch = $Window.FindName("BtnLaunch")
$BtnLaunchBg = $Window.FindName("BtnLaunchBg")
$BtnLaunchVnc = $Window.FindName("BtnLaunchVnc")
$BtnDeleteDisk = $Window.FindName("BtnDeleteDisk")
$BtnDeleteIso = $Window.FindName("BtnDeleteIso")
$BtnPurge = $Window.FindName("BtnPurge")
$TxtStatus = $Window.FindName("TxtStatus")

# Load Target Data
$ModuleRoot = Join-Path $PSScriptRoot "modules"
$TargetsModule = Join-Path $ModuleRoot "targets.ps1"
if (Test-Path $TargetsModule) {
    . $TargetsModule
    foreach ($Target in $IsoMatrix) {
        [void]$TargetListView.Items.Add([pscustomobject]@{
                Id       = $Target.Id
                Name     = $Target.Name
                OsFamily = $Target.OsFamily
                DiskSize = if ($Target.DiskSize) { $Target.DiskSize } else { "40G" }
            })
    }
}
else {
    $TxtStatus.Text = "[FAIL] Could not find $TargetsModule"
    $TxtStatus.Foreground = "#FF5555"
}

# Helper Function
function Invoke-StartScript {
    param(
        [string]$TargetId,
        [string[]]$ArgsList
    )
    
    $ScriptPath = Join-Path $PSScriptRoot "start.ps1"
    if (-not (Test-Path $ScriptPath)) {
        $TxtStatus.Text = "[FAIL] Could not find start.ps1"
        $TxtStatus.Foreground = "#FF5555"
        return
    }

    $AllArgs = @($TargetId) + $ArgsList
    $TxtStatus.Text = "Running: .\start.ps1 $TargetId $($ArgsList -join ' ')"
    $TxtStatus.Foreground = "#00E5FF"
    
    # Run inside a new PowerShell window so interactive prompts / CLI UI works
    $ProcArgs = @("-NoExit", "-ExecutionPolicy", "Bypass", "-File", "`"$ScriptPath`"") + $AllArgs
    Start-Process -FilePath $PowershellDir -ArgumentList $ProcArgs
}

# Event Wiring
$BtnLaunch.Add_Click({
        if ($TargetListView.SelectedItem) {
            Invoke-StartScript -TargetId $TargetListView.SelectedItem.Id
        }
        else {
            $TxtStatus.Text = "Please select a target first."
            $TxtStatus.Foreground = "#FFAA00"
        }
    })

$BtnLaunchBg.Add_Click({
        if ($TargetListView.SelectedItem) {
            Invoke-StartScript -TargetId $TargetListView.SelectedItem.Id -ArgsList @("-Background")
        }
        else {
            $TxtStatus.Text = "Please select a target first."
            $TxtStatus.Foreground = "#FFAA00"
        }
    })

$BtnLaunchVnc.Add_Click({
        if ($TargetListView.SelectedItem) {
            Invoke-StartScript -TargetId $TargetListView.SelectedItem.Id -ArgsList @("-Vnc")
        }
        else {
            $TxtStatus.Text = "Please select a target first."
            $TxtStatus.Foreground = "#FFAA00"
        }
    })

$BtnDeleteDisk.Add_Click({
        if ($TargetListView.SelectedItem) {
            Invoke-StartScript -TargetId $TargetListView.SelectedItem.Id -ArgsList @("-DeleteDisk")
        }
        else {
            $TxtStatus.Text = "Please select a target first."
            $TxtStatus.Foreground = "#FFAA00"
        }
    })

$BtnDeleteIso.Add_Click({
        if ($TargetListView.SelectedItem) {
            Invoke-StartScript -TargetId $TargetListView.SelectedItem.Id -ArgsList @("-DeleteIso")
        }
        else {
            $TxtStatus.Text = "Please select a target first."
            $TxtStatus.Foreground = "#FFAA00"
        }
    })

$BtnPurge.Add_Click({
        if ($TargetListView.SelectedItem) {
            Invoke-StartScript -TargetId $TargetListView.SelectedItem.Id -ArgsList @("-Purge")
        }
        else {
            $TxtStatus.Text = "Please select a target first."
            $TxtStatus.Foreground = "#FFAA00"
        }
    })

# Display Window
$Window.ShowDialog() | Out-Null
