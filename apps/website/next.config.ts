import type { NextConfig } from "next";

// GitHub Pages serves this repository beneath /Your-Dawnloader.
// Leave NEXT_PUBLIC_BASE_PATH unset for local development or a domain-root host.
const basePath = process.env.NEXT_PUBLIC_BASE_PATH?.trim() || "";

const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  basePath: basePath || undefined,
  images: { unoptimized: true },
  poweredByHeader: false,
  reactStrictMode: true,
};

export default nextConfig;
