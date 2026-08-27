// @ts-nocheck
import { mount } from "svelte";
import SettingsApp from "./lib/SettingsApp.svelte";

const app = mount(SettingsApp, { target: document.body });

export default app;
