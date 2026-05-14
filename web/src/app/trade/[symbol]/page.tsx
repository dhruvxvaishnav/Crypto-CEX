import { TradingScreen } from "@/components/trading/TradingScreen";

interface TradePageProps {
  params: Promise<{
    symbol: string;
  }>;
}

export default async function TradePage({ params }: TradePageProps) {
  const { symbol } = await params;
  return <TradingScreen initialSymbol={symbol} />;
}
