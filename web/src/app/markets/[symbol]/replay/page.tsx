import { ReplayScreen } from "@/components/replay/ReplayScreen";

interface ReplayPageProps {
  params: Promise<{
    symbol: string;
  }>;
}

export default async function ReplayPage({ params }: ReplayPageProps) {
  const { symbol } = await params;
  return <ReplayScreen initialSymbol={symbol} />;
}
