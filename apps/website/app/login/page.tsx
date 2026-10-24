import type { Metadata } from "next";
import LoginClient from "./login-client";

export const metadata: Metadata = {
  title: "Sign in or create account",
  description: "Sign in with Google, Apple, or email to access your Your Dawnloader account.",
};

export default function LoginPage() {
  return <LoginClient />;
}
