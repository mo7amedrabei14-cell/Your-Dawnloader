import type { Metadata } from "next";
import ConnectDeviceClient from "./connect-device-client";

export const metadata: Metadata = {
  title: "Connect a device",
  description: "Approve a Your Dawnloader desktop device to sync download-history metadata to your account.",
};

export default async function ConnectDevicePage({
  searchParams,
}: {
  searchParams: Promise<{ user_code?: string | string[] }>;
}) {
  const params = await searchParams;
  const code = Array.isArray(params.user_code) ? params.user_code[0] : params.user_code;
  return <ConnectDeviceClient initialCode={code || ""} />;
}
