--
-- PostgreSQL database dump
--

\restrict dtPSxyaF3UMNzfzRKHrFZf5zIe3ttM6QdjiI8okpw1OkRJdLrDbYwOgiVnEyrCY

-- Dumped from database version 18.1
-- Dumped by pg_dump version 18.1

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: discussion_posts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.discussion_posts (
    id uuid NOT NULL,
    thread_id uuid NOT NULL,
    parent_id uuid,
    depth smallint NOT NULL,
    author_id uuid NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    edited_at timestamp with time zone
);


ALTER TABLE public.discussion_posts OWNER TO postgres;

--
-- Data for Name: discussion_posts; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.discussion_posts (id, thread_id, parent_id, depth, author_id, title, body, created_at, edited_at) FROM stdin;
\.


--
-- Name: discussion_posts discussion_posts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.discussion_posts
    ADD CONSTRAINT discussion_posts_pkey PRIMARY KEY (id);


--
-- Name: discussion_posts discussion_posts_author_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.discussion_posts
    ADD CONSTRAINT discussion_posts_author_id_fkey FOREIGN KEY (author_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict dtPSxyaF3UMNzfzRKHrFZf5zIe3ttM6QdjiI8okpw1OkRJdLrDbYwOgiVnEyrCY

