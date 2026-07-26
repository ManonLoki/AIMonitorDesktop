import {
  AppShell,
  Burger,
  Group,
  Image,
  NavLink,
  ScrollArea,
  Text,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  Link,
  Outlet,
  createRootRoute,
  createRoute,
  createRouter,
  useMatchRoute,
} from "@tanstack/react-router";
import { DashboardPage } from "./pages/DashboardPage";
import { TechStackPage } from "./pages/TechStackPage";

function RootLayout() {
  const [opened, { toggle }] = useDisclosure();
  const matchRoute = useMatchRoute();

  return (
    <AppShell
      header={{ height: 64 }}
      navbar={{ width: 248, breakpoint: "sm", collapsed: { mobile: !opened } }}
      padding="xl"
    >
      <AppShell.Header>
        <Group h="100%" px="lg" justify="space-between">
          <Group>
            <Burger
              opened={opened}
              onClick={toggle}
              hiddenFrom="sm"
              size="sm"
            />
            <Image
              src="/branding/aimonitor-logo.png"
              alt="AIMonitorDesktop Logo"
              className="brand-logo"
            />
            <div>
              <Text fw={800} lh={1.1}>
                AIMonitorDesktop
              </Text>
              <Text size="xs" c="dimmed">
                AI监控平台桌面版
              </Text>
            </div>
          </Group>
          <Group gap="xs">
            <span className="status-dot" />
            <Text size="sm" c="dimmed">
              平台运行正常
            </Text>
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <AppShell.Section grow component={ScrollArea}>
          <Text size="xs" fw={700} c="dimmed" px="sm" mb="xs">
            工作台
          </Text>
          <NavLink
            component={Link}
            to="/"
            label="监控总览"
            active={Boolean(matchRoute({ to: "/", fuzzy: false }))}
            onClick={() => opened && toggle()}
          />
          <NavLink
            component={Link}
            to="/tech-stack"
            label="技术栈"
            active={Boolean(matchRoute({ to: "/tech-stack" }))}
            onClick={() => opened && toggle()}
          />
        </AppShell.Section>
        <AppShell.Section>
          <Text c="dimmed" size="xs" px="sm">
            v1.0.1 · ManonLoki
          </Text>
        </AppShell.Section>
      </AppShell.Navbar>

      <AppShell.Main>
        <Outlet />
      </AppShell.Main>
    </AppShell>
  );
}

const rootRoute = createRootRoute({
  component: RootLayout,
  notFoundComponent: () => (
    <div>
      <Text fw={700}>页面不存在</Text>
      <Link to="/">返回监控总览</Link>
    </div>
  ),
});

const dashboardRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

const techStackRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/tech-stack",
  component: TechStackPage,
});

const routeTree = rootRoute.addChildren([dashboardRoute, techStackRoute]);

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  scrollRestoration: true,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
