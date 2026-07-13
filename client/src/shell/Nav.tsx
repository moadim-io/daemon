import { NavLink } from "react-router-dom";

const TABS = [
  { to: "/", label: "Overview", end: true },
  { to: "/routines", label: "Routines", end: false },
  { to: "/heatmap", label: "Heatmap", end: false },
  { to: "/settings", label: "Settings", end: false },
];

export function Nav() {
  return (
    <nav className="app-nav">
      {TABS.map((tab) => (
        <NavLink
          key={tab.to}
          to={tab.to}
          end={tab.end}
          className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
        >
          {tab.label}
        </NavLink>
      ))}
    </nav>
  );
}
