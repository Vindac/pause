// @ts-nocheck —— Svelte 编译器插件处理 .svelte 导入
import { mount } from "svelte";
import ReminderApp from "./lib/ReminderApp.svelte";

const app = mount(ReminderApp, { target: document.body });

export default app;
