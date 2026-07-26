import {
  Badge,
  Button,
  Card,
  Group,
  Progress,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  ThemeIcon,
  Title,
} from "@mantine/core";
import { useAtom } from "jotai";
import {
  monitoringEnabledAtom,
  selectedEnvironmentAtom,
} from "../state/app";

const metrics = [
  { label: "活跃模型", value: "12", detail: "2 个正在训练", color: "cyan" },
  { label: "今日请求", value: "48.6K", detail: "+12.4% 较昨日", color: "indigo" },
  { label: "平均延迟", value: "286ms", detail: "P95 742ms", color: "violet" },
  { label: "异常事件", value: "3", detail: "1 个待处理", color: "orange" },
];

const services = [
  { name: "推理网关", status: "运行中", load: 68, color: "cyan" },
  { name: "模型服务", status: "运行中", load: 44, color: "indigo" },
  { name: "告警中心", status: "运行中", load: 27, color: "teal" },
];

export function DashboardPage() {
  const [monitoringEnabled, setMonitoringEnabled] = useAtom(
    monitoringEnabledAtom,
  );
  const [environment, setEnvironment] = useAtom(selectedEnvironmentAtom);

  return (
    <Stack gap="xl">
      <Group justify="space-between" align="flex-start">
        <div>
          <Text c="dimmed" size="sm" fw={600}>
            AI MONITORING OVERVIEW
          </Text>
          <Title order={1}>监控总览</Title>
          <Text c="dimmed" mt={4}>
            实时掌握模型、服务与基础设施运行状态
          </Text>
        </div>
        <Group>
          <Select
            aria-label="运行环境"
            data={[
              { value: "production", label: "生产环境" },
              { value: "staging", label: "预发布环境" },
            ]}
            value={environment}
            onChange={(value) => {
              if (value === "production" || value === "staging") {
                setEnvironment(value);
              }
            }}
            allowDeselect={false}
          />
          <Switch
            checked={monitoringEnabled}
            onChange={(event) =>
              setMonitoringEnabled(event.currentTarget.checked)
            }
            label="实时监控"
            color="cyan"
          />
        </Group>
      </Group>

      <SimpleGrid cols={{ base: 1, sm: 2, xl: 4 }}>
        {metrics.map((metric) => (
          <Card key={metric.label} withBorder radius="lg" padding="lg">
            <Group justify="space-between">
              <Text c="dimmed" size="sm">
                {metric.label}
              </Text>
              <ThemeIcon variant="light" color={metric.color} radius="xl">
                <span className="metric-pulse" />
              </ThemeIcon>
            </Group>
            <Text fz={32} fw={700} mt="md">
              {metric.value}
            </Text>
            <Text c="dimmed" size="xs" mt={4}>
              {metric.detail}
            </Text>
          </Card>
        ))}
      </SimpleGrid>

      <SimpleGrid cols={{ base: 1, lg: 3 }}>
        <Card withBorder radius="lg" padding="xl" className="service-card">
          <Group justify="space-between" mb="xl">
            <div>
              <Title order={3}>服务状态</Title>
              <Text c="dimmed" size="sm">
                核心组件实时负载
              </Text>
            </div>
            <Badge color="teal" variant="light">
              全部正常
            </Badge>
          </Group>
          <Stack gap="lg">
            {services.map((service) => (
              <div key={service.name}>
                <Group justify="space-between" mb={8}>
                  <Group gap="xs">
                    <span className="status-dot" />
                    <Text fw={600}>{service.name}</Text>
                  </Group>
                  <Text c="dimmed" size="xs">
                    {service.status} · {service.load}%
                  </Text>
                </Group>
                <Progress
                  value={service.load}
                  color={service.color}
                  radius="xl"
                  size="sm"
                />
              </div>
            ))}
          </Stack>
        </Card>

        <Card withBorder radius="lg" padding="xl">
          <Title order={3}>快速操作</Title>
          <Text c="dimmed" size="sm" mb="xl">
            管理监控任务与告警策略
          </Text>
          <Stack>
            <Button variant="light" color="cyan" fullWidth>
              新建监控任务
            </Button>
            <Button variant="default" fullWidth>
              配置告警规则
            </Button>
            <Button variant="default" fullWidth>
              查看运行日志
            </Button>
          </Stack>
        </Card>
      </SimpleGrid>
    </Stack>
  );
}
