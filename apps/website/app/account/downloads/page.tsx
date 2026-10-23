import type { Metadata } from "next";
import MyDownloadsClient from "./my-downloads-client";
export const metadata: Metadata = { title: "My Downloads", description: "A private account library for your download history." };
export default function MyDownloadsPage() { return <MyDownloadsClient />; }
