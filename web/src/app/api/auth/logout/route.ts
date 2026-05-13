import { cookies } from "next/headers";
import { NextResponse } from "next/server";

export async function POST(): Promise<NextResponse> {
  const cookieStore = await cookies();
  cookieStore.delete("aether_refresh");
  return NextResponse.json({ ok: true });
}
