import { queryOptions } from "@tanstack/react-query";
import { apiClient } from "./client";

export interface PlatformHealth {
  status: "healthy" | "degraded" | "offline";
  checkedAt: string;
}

export const platformHealthQuery = () =>
  queryOptions({
    queryKey: ["platform-health"],
    queryFn: async () => {
      const response = await apiClient.get<PlatformHealth>("/health");
      return response.data;
    },
    staleTime: 30_000,
    retry: 2,
  });
