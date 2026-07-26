import { Badge, Card, Group, SimpleGrid, Stack, Text, Title } from "@mantine/core";

const stack = [
  ["桌面运行时", "Tauri 2"],
  ["视图层", "React 19 + TypeScript"],
  ["UI 组件", "Mantine 9"],
  ["路由", "TanStack Router"],
  ["服务端状态", "TanStack Query + Axios"],
  ["客户端状态", "Jotai"],
  ["构建工具", "Vite 7"],
];

export function TechStackPage() {
  return (
    <Stack gap="xl">
      <div>
        <Text c="dimmed" size="sm" fw={600}>
          ARCHITECTURE
        </Text>
        <Title order={1}>技术栈</Title>
        <Text c="dimmed" mt={4}>
          AIMonitorDesktop 已固化的基础技术选型
        </Text>
      </div>
      <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }}>
        {stack.map(([category, technology]) => (
          <Card key={category} withBorder radius="lg" padding="lg">
            <Group justify="space-between" align="flex-start">
              <Text c="dimmed" size="sm">
                {category}
              </Text>
              <Badge variant="dot" color="cyan">
                Locked
              </Badge>
            </Group>
            <Title order={3} mt="xl">
              {technology}
            </Title>
          </Card>
        ))}
      </SimpleGrid>
    </Stack>
  );
}
