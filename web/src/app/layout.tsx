import type { Metadata, Viewport } from "next";
import { Inter } from "next/font/google";

import { Providers } from "./providers";
import "./globals.css";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-sans",
  display: "swap",
});

export const metadata: Metadata = {
  title: {
    template: "%s | Aether",
    default: "Aether — Crypto Exchange",
  },
  description:
    "Production-architecture crypto exchange. Spot trading with sub-millisecond matching engine.",
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: dark)", color: "#09090b" },
    { media: "(prefers-color-scheme: light)", color: "#fafafa" },
  ],
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" suppressHydrationWarning className={`h-full ${inter.variable}`}>
      <body className="h-full font-sans antialiased">
        <Providers>
          {/* Skip-to-main-content for accessibility (PRD §17.5) */}
          <a
            href="#main-content"
            className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:rounded focus:bg-emerald-500 focus:px-3 focus:py-2 focus:text-zinc-950 focus:text-sm focus:font-medium"
          >
            Skip to main content
          </a>
          {children}
        </Providers>
      </body>
    </html>
  );
}
