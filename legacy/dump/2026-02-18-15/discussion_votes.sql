--
-- PostgreSQL database dump
--

\restrict 2Iaqymd8unrp9Y0IP0hW6102TO44676fFJfnEFrFuegYxFsdiS0WxTQoItUWLyd

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
-- Name: discussion_votes; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.discussion_votes (
    post_id uuid NOT NULL,
    user_id uuid NOT NULL,
    value smallint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.discussion_votes OWNER TO postgres;

--
-- Data for Name: discussion_votes; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.discussion_votes (post_id, user_id, value, created_at) FROM stdin;
\.


--
-- Name: discussion_votes discussion_votes_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.discussion_votes
    ADD CONSTRAINT discussion_votes_pkey PRIMARY KEY (post_id, user_id);


--
-- Name: discussion_votes discussion_votes_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.discussion_votes
    ADD CONSTRAINT discussion_votes_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.discussion_posts(id) ON DELETE CASCADE;


--
-- Name: discussion_votes discussion_votes_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.discussion_votes
    ADD CONSTRAINT discussion_votes_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict 2Iaqymd8unrp9Y0IP0hW6102TO44676fFJfnEFrFuegYxFsdiS0WxTQoItUWLyd

