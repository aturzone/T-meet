import { lazy, Suspense } from "react";
import { createBrowserRouter, RouterProvider } from "react-router-dom";

import { Spinner } from "../components/ui/Spinner";

const Landing = lazy(() => import("../pages/Landing"));
const Room = lazy(() => import("../pages/Room"));
const Setup = lazy(() => import("../pages/Setup"));
const NotFound = lazy(() => import("../pages/NotFound"));

const router = createBrowserRouter([
  { path: "/", element: withFallback(<Landing />) },
  { path: "/r/:id", element: withFallback(<Room />) },
  { path: "/setup", element: withFallback(<Setup />) },
  { path: "*", element: withFallback(<NotFound />) },
]);

function withFallback(node: React.ReactNode) {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen flex items-center justify-center">
          <Spinner />
        </div>
      }
    >
      {node}
    </Suspense>
  );
}

export function Router() {
  return <RouterProvider router={router} />;
}
