import type { Metadata } from "next";
import AdminUsersClient from "./admin-users-client";

export const metadata: Metadata = { title: "User administration" };

export default function AdminUsersPage() {
  return <AdminUsersClient />;
}
