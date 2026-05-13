interface SkeletonProps {
  className?: string;
}

export function Skeleton({ className = "" }: SkeletonProps) {
  return <div aria-hidden className={`animate-pulse rounded bg-zinc-800 ${className}`} />;
}
