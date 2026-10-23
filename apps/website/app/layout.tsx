import type { Metadata } from "next";
import "./globals.css";
import MotionShell from "./motion-shell";

export const metadata: Metadata = {
  title: { default: "Your Dawnloader — Media, under your control", template: "%s · Your Dawnloader" },
  description: "A privacy-first media discovery and download manager for desktop and browser. Save only media you are authorized to download.",
  applicationName: "Your Dawnloader",
  category: "software",
  robots: { index: true, follow: true },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body><MotionShell />{children}</body>
    </html>
  );
}
