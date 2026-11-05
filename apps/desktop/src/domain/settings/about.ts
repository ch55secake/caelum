export type AppInfo = {
  name: string;
  version: string;
  platform: string;
  architecture: string;
  repositoryUrl: string;
  releasesUrl: string;
  license: string;
  licenseUrl: string;
};

export type UpdateCheck = {
  currentVersion: string;
  latestVersion: string;
  status: "available" | "upToDate" | "ahead";
  releaseUrl: string;
  downloadUrl: string | null;
};
