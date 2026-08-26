import SwiftUI

/// 设置窗口（修改立即保存）：
/// 通用（语言）/ 提醒（间隔·时长·延迟）/ 图片（预览·切换）/ 系统 / 提醒窗口。
struct SettingsView: View {
    @EnvironmentObject private var vm: SettingsViewModel
    @EnvironmentObject private var l10n: LocalizationStore

    var body: some View {
        Form {
            Section(l10n.t(.sectionGeneral)) {
                Picker(l10n.t(.languageLabel), selection: languageBinding) {
                    ForEach(AppLanguage.allCases) { language in
                        Text(language.displayName).tag(language)
                    }
                }

                LabeledContent(l10n.t(.versionLabel), value: AppInfo.version)
            }

            Section(l10n.t(.sectionReminder)) {
                Picker(l10n.t(.intervalLabel), selection: $vm.reminderIntervalMinutes) {
                    ForEach(SettingsViewModel.quickIntervals, id: \.self) { minutes in
                        Text(l10n.t(.minutes(minutes))).tag(minutes)
                    }
                    if vm.isCustomInterval {
                        Text(l10n.t(.customMinutes(vm.reminderIntervalMinutes)))
                            .tag(vm.reminderIntervalMinutes)
                    }
                }
                .onChange(of: vm.reminderIntervalMinutes) { _ in
                    // 间隔变化由 ReminderService 订阅 SettingsStore 自动重开工作周期
                }

                if vm.isCustomInterval || vm.reminderIntervalMinutes < 30 || vm.reminderIntervalMinutes > 60 {
                    Stepper(l10n.t(.customIntervalText(vm.reminderIntervalMinutes)),
                            value: $vm.reminderIntervalMinutes,
                            in: ReminderSettings.intervalRange)
                }

                Stepper(l10n.t(.breakDurationText(vm.breakDurationMinutes)),
                        value: $vm.breakDurationMinutes,
                        in: ReminderSettings.breakDurationRange)

                Toggle(l10n.t(.usageTimingLabel), isOn: $vm.activityBasedTiming)
                if vm.activityBasedTiming {
                    Picker(l10n.t(.idleThresholdLabel), selection: $vm.idleThresholdMinutes) {
                        ForEach(SettingsViewModel.quickIdleThresholds, id: \.self) { minutes in
                            Text(l10n.t(.minutes(minutes))).tag(minutes)
                        }
                    }
                }

                Toggle(l10n.t(.autoStartBreakLabel), isOn: $vm.autoStartBreak)
                if vm.autoStartBreak {
                    Picker(l10n.t(.autoStartBreakDelayLabel), selection: $vm.autoStartBreakDelaySeconds) {
                        ForEach(SettingsViewModel.quickAutoBreakDelays, id: \.self) { seconds in
                            Text(l10n.t(.seconds(seconds))).tag(seconds)
                        }
                    }
                }

                Picker(l10n.t(.snoozeDurationLabel), selection: snoozeBinding) {
                    ForEach(SettingsViewModel.quickSnoozeMinutes, id: \.self) { minutes in
                        Text(l10n.t(.minutes(minutes))).tag(minutes)
                    }
                    if vm.isCustomSnooze {
                        Text(l10n.t(.customMinutes(vm.snoozeMinutes))).tag(vm.snoozeMinutes)
                    }
                }

                if vm.isCustomSnooze {
                    // 原生 TextField 独立表单行（标准可交互形态；回车提交并钳制到 1–15）
                    TextField(l10n.t(.snoozeCustomPrefix), text: $vm.customSnoozeText)
                        .onSubmit { vm.commitCustomSnoozeText() }
                        .onChange(of: vm.customSnoozeText) { _ in
                            vm.commitCustomSnoozeText()   // 输入完成即钳制生效
                        }
                }

                Text(l10n.t(.snoozeCaption))
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if vm.activityBasedTiming {
                    Text(l10n.t(.usageTimingHint))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                if vm.autoStartBreak {
                    Text(l10n.t(.autoStartBreakHint))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Section(l10n.t(.sectionWallpaper)) {
                if let preview = vm.wallpaperPreview {
                    Image(nsImage: preview)
                        .resizable()
                        .aspectRatio(contentMode: .fill)
                        .frame(height: 110)
                        .frame(maxWidth: .infinity)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .strokeBorder(.quaternary, lineWidth: 1)
                        )
                }

                Button(l10n.t(.switchWallpaper)) {
                    DebugLog.log("tapped 切换图片 button")
                    vm.switchWallpaper()
                }
            }

            Section(l10n.t(.sectionSystem)) {
                Toggle(l10n.t(.launchAtLoginLabel), isOn: $vm.launchAtLogin)
                Toggle(l10n.t(.soundLabel), isOn: $vm.soundEnabled)
                Toggle(l10n.t(.overlayLabel), isOn: $vm.overlayOtherWindows)
                    .onChange(of: vm.overlayOtherWindows) { _ in
                        DebugLog.log("toggle 提醒时覆盖其他窗口 changed")
                    }
            }

            Section(l10n.t(.sectionWindow)) {
                HStack {
                    Text(l10n.t(.windowOpacityLabel))
                    Slider(value: $vm.reminderWindowOpacity,
                           in: ReminderSettings.windowOpacityRange)
                    Text(vm.opacityPercentText)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                        .frame(width: 44, alignment: .trailing)
                }
                Text(l10n.t(.windowOpacityHint))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 540, minHeight: 620)
    }

    /// 语言选择（经 ViewModel 落盘）
    private var languageBinding: Binding<AppLanguage> {
        Binding(get: { vm.language }, set: { vm.setLanguage($0) })
    }

    /// 延迟时间选择：快捷值同步自定义输入框
    private var snoozeBinding: Binding<Int> {
        Binding(
            get: { vm.snoozeMinutes },
            set: { vm.selectSnoozeMinutes($0) }
        )
    }
}
